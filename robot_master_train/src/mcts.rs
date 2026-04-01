use board_game::board::{Board as _, Outcome};
use robot_master_arena::player::Bot;
use robot_master_core::game::{GameState, Move};

/// Produces (policy, value) estimates for a game state.
///
/// Two intended implementations:
/// - `RolloutEval<B: Bot>`: plays a bot to terminal, returns outcome. Phase 2.
/// - `NnEval`: ONNX inference. Phase 3.
pub trait Evaluator<const N: usize>
where
	[(); N * N]:, {
	fn evaluate(&self, state: &GameState<N>) -> Evaluation;
}
/// Evaluation result for a leaf node: policy prior over moves and a value estimate.
pub struct Evaluation {
	/// (move, prior probability) pairs. Must cover all legal moves. Need not be normalized.
	pub policy: Vec<(Move, f32)>,
	/// Position value from the perspective of the player to move. In [-1, 1].
	pub value: f32,
}

/// Evaluator that plays a game to completion using a Bot, then returns the outcome.
pub struct RolloutEval<B> {
	bot: B,
}

impl<B> RolloutEval<B> {
	pub fn new(bot: B) -> Self {
		Self { bot }
	}
}

impl<B, const N: usize> Evaluator<N> for RolloutEval<B>
where
	B: Bot<N> + Clone,
	[(); N * N]:,
{
	fn evaluate(&self, state: &GameState<N>) -> Evaluation {
		let player = state.turn;
		let policy: Vec<(Move, f32)> = state.valid_moves().map(|m| (m, 1.0)).collect();

		let mut sim = state.clone();
		let mut bot = self.bot.clone();
		while sim.outcome().is_none() {
			let mv = bot.choose_move(&sim);
			sim.play(mv).expect("bot produced illegal move");
		}

		let value = outcome_value(sim.outcome().expect("game should be done"), player);
		Evaluation { policy, value }
	}
}

impl<const N: usize> Evaluator<N> for Box<dyn Evaluator<N>>
where
	[(); N * N]:,
{
	fn evaluate(&self, state: &GameState<N>) -> Evaluation {
		(**self).evaluate(state)
	}
}

/// +1 if player won, -1 if lost, 0 if draw.
fn outcome_value(outcome: Outcome, player: board_game::board::Player) -> f32 {
	match outcome {
		Outcome::WonBy(winner) if winner == player => 1.0,
		Outcome::WonBy(_) => -1.0,
		Outcome::Draw => 0.0,
	}
}

// --- Tree ---

pub(crate) struct Edge {
	pub(crate) mv: Move,
	pub(crate) prior: f32,
	/// Index into `Tree::nodes`, or `u32::MAX` if unexpanded.
	pub(crate) child: u32,
}

pub(crate) struct Node {
	/// Total value accumulated through this node (from the perspective of the player who moved *to* this node).
	pub(crate) total_value: f64,
	pub(crate) visit_count: u32,
	pub(crate) edges: Vec<Edge>,
}

impl Node {
	pub(crate) fn q(&self) -> f64 {
		if self.visit_count == 0 { 0.0 } else { self.total_value / self.visit_count as f64 }
	}
}

pub(crate) struct Tree {
	pub(crate) nodes: Vec<Node>,
}

impl Tree {
	/// Create a tree with a root node already expanded from the given moves and priors.
	/// Used by Gumbel search to set up the root without going through `Evaluation`.
	pub(crate) fn new_with_root(root_value: f32, moves: &[Move], priors: &[f32]) -> Self {
		let edges: Vec<Edge> = moves.iter().zip(priors).map(|(&mv, &prior)| Edge { mv, prior, child: u32::MAX }).collect();
		Self {
			nodes: vec![Node {
				total_value: root_value as f64,
				visit_count: 1,
				edges,
			}],
		}
	}

	fn expand(&mut self, eval: Evaluation) -> u32 {
		let prior_sum: f32 = eval.policy.iter().map(|(_, p)| *p).sum();
		let edges: Vec<Edge> = eval
			.policy
			.into_iter()
			.map(|(mv, p)| Edge {
				mv,
				prior: if prior_sum > 0.0 { p / prior_sum } else { 1.0 },
				child: u32::MAX,
			})
			.collect();
		let idx = self.nodes.len() as u32;
		self.nodes.push(Node {
			total_value: eval.value as f64,
			visit_count: 1,
			edges,
		});
		idx
	}

	fn expand_terminal(&mut self, value: f32) -> u32 {
		let idx = self.nodes.len() as u32;
		self.nodes.push(Node {
			total_value: value as f64,
			visit_count: 1,
			edges: Vec::new(),
		});
		idx
	}

	/// Raw Q-value of root edge `action_idx` (negated child Q, from root's perspective).
	pub(crate) fn root_q_raw(&self, action_idx: usize) -> f32 {
		let edge = &self.nodes[0].edges[action_idx];
		if edge.child == u32::MAX {
			return 0.0;
		}
		-self.nodes[edge.child as usize].q() as f32
	}

	/// Normalized Q in [0,1] using tree min/max range.
	pub(crate) fn root_q(&self, action_idx: usize) -> f32 {
		let (q_min, q_max) = self.q_range(self.nodes[0].total_value as f32);
		let raw = self.root_q_raw(action_idx);
		if (q_max - q_min).abs() < 1e-8 {
			0.5
		} else {
			((raw - q_min) / (q_max - q_min)).clamp(0.0, 1.0)
		}
	}

	/// Whether root edge `action_idx` has been visited at all.
	pub(crate) fn root_visited(&self, action_idx: usize) -> bool {
		self.nodes[0].edges[action_idx].child != u32::MAX
	}

	/// Visit count of the most-visited root edge.
	pub(crate) fn max_root_visits(&self) -> u32 {
		self.nodes[0]
			.edges
			.iter()
			.map(|e| if e.child == u32::MAX { 0 } else { self.nodes[e.child as usize].visit_count })
			.max()
			.unwrap_or(0)
	}

	/// (min, max) Q range seen in the tree, anchored by v̂_π.
	pub(crate) fn q_range(&self, v_pi: f32) -> (f32, f32) {
		let mut q_min = v_pi;
		let mut q_max = v_pi;
		for (i, _) in self.nodes[0].edges.iter().enumerate() {
			if self.root_visited(i) {
				let raw = self.root_q_raw(i);
				q_min = q_min.min(raw);
				q_max = q_max.max(raw);
			}
		}
		(q_min, q_max)
	}
}

/// One simulation: select -> expand -> backpropagate.
///
/// `forced_root_action`: if `Some(i)`, always take root edge `i` on the first step (Gumbel).
/// If `None`, use PUCT at the root as normal.
pub(crate) fn simulate<const N: usize>(tree: &mut Tree, node_idx: u32, forced_root_action: Option<usize>, state: &GameState<N>, evaluator: &impl Evaluator<N>, c_puct: f32)
where
	[(); N * N]:, {
	let mut path: Vec<u32> = Vec::new(); // node indices along the path
	let mut current = node_idx;
	let mut sim_state = state.clone();
	let mut is_root = true;

	//LOOP: embed termination condition
	while let Some(node) = tree.nodes.get(current as usize).filter(|n| !n.edges.is_empty()) {
		let best_edge_idx = if is_root {
			is_root = false;
			match forced_root_action {
				Some(idx) => idx,
				None => select_edge(&tree.nodes, node, c_puct),
			}
		} else {
			select_edge(&tree.nodes, node, c_puct)
		};

		path.push(current);
		let mv = tree.nodes[current as usize].edges[best_edge_idx].mv;
		sim_state.play(mv).expect("search selected illegal move");

		let child = tree.nodes[current as usize].edges[best_edge_idx].child;
		if child == u32::MAX {
			let child_idx = expand_state(tree, &sim_state, evaluator);
			tree.nodes[current as usize].edges[best_edge_idx].child = child_idx;
			backpropagate(tree, &path, tree.nodes[child_idx as usize].total_value);
			return;
		}

		current = child;
	}

	// Re-visited a terminal node.
	let value = sim_state.outcome().map(|o| outcome_value(o, sim_state.turn)).unwrap_or(0.0);
	backpropagate(tree, &path, value as f64);
}

fn select_edge(nodes: &[Node], node: &Node, c_puct: f32) -> usize {
	(0..node.edges.len())
		.max_by(|&a, &b| {
			let sa = edge_uct(nodes, &node.edges[a], node, c_puct);
			let sb = edge_uct(nodes, &node.edges[b], node, c_puct);
			sa.partial_cmp(&sb).expect("NaN in UCT")
		})
		.expect("edges is non-empty")
}

fn edge_uct(nodes: &[Node], edge: &Edge, parent: &Node, c_puct: f32) -> f64 {
	// For unvisited children, use the parent's current mean Q as the value estimate rather
	// than 0. MiniZero §III-B (arxiv 2310.11305): "Q̂(s) = Q_Σ(s) / (N_Σ(s) + 1)" —
	// initialising to 0 systematically over-explores bad moves early and under-explores
	// good ones; the parent mean is a neutral baseline that avoids this bias. The +1
	// denominator (virtual visit) is absorbed into our visit_count=0 path below.
	let (child_q, child_visits) = if edge.child == u32::MAX {
		(-parent.q(), 0) // negated: parent Q is from parent's mover perspective
	} else {
		let child = &nodes[edge.child as usize];
		(-child.q(), child.visit_count)
	};
	// Q + c * P * sqrt(N_parent) / (1 + N_child)
	child_q + c_puct as f64 * edge.prior as f64 * (parent.visit_count as f64).sqrt() / (1.0 + child_visits as f64)
}

fn expand_state<const N: usize>(tree: &mut Tree, state: &GameState<N>, evaluator: &impl Evaluator<N>) -> u32
where
	[(); N * N]:, {
	if let Some(outcome) = state.outcome() {
		return tree.expand_terminal(outcome_value(outcome, state.turn));
	}
	tree.expand(evaluator.evaluate(state))
}

/// Walk back up the path, negating at each level (zero-sum).
fn backpropagate(tree: &mut Tree, path: &[u32], leaf_value: f64) {
	let mut value = leaf_value;
	for &n_idx in path.iter().rev() {
		value = -value;
		let n = &mut tree.nodes[n_idx as usize];
		n.visit_count += 1;
		n.total_value += value;
	}
}
