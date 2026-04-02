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
	[(); N * N]:,
	[(); N + 1]:, {
	fn evaluate(&self, state: &GameState<N>) -> Evaluation;

	/// Evaluate a batch of states. Default impl calls `evaluate` in a loop;
	/// `NnEval` overrides with a single batched ONNX call.
	fn evaluate_batch(&self, states: &[GameState<N>]) -> Vec<Evaluation> {
		states.iter().map(|s| self.evaluate(s)).collect()
	}
}
/// Unified interface for search-wrapped bots (Vanilla MCTS, Gumbel).
/// Implementors wrap an `Evaluator<N>` and expose construction from a sim count.
pub trait SearchBot<E, const N: usize>: Bot<N>
where
	[(); N * N]:,
	[(); N + 1]:, {
	fn with_sims(evaluator: E, sims: u32) -> Self;
}
/// Evaluation result for a leaf node: policy prior over moves and a value estimate.
#[derive(Clone)]
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
	[(); N + 1]:,
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
	[(); N + 1]:,
{
	fn evaluate(&self, state: &GameState<N>) -> Evaluation {
		(**self).evaluate(state)
	}
}

/// Vanilla UCT-MCTS bot. Runs `sims` full simulations from the root, picks most-visited child.
pub struct VanillaMcts<E> {
	evaluator: E,
	sims: u32,
	c_puct: f32,
}
impl<E> VanillaMcts<E> {
	pub fn new(evaluator: E, sims: u32) -> Self {
		Self { evaluator, sims, c_puct: 1.41 }
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

	/// Visit count of root edge `action_idx` (0 if unvisited).
	pub(crate) fn root_visit_count(&self, action_idx: usize) -> u32 {
		let edge = &self.nodes[0].edges[action_idx];
		if edge.child == u32::MAX { 0 } else { self.nodes[edge.child as usize].visit_count }
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

impl Default for Tree {
	fn default() -> Self {
		Self { nodes: Vec::new() }
	}
}

/// Result of walking the tree to a leaf during selection.
pub(crate) enum SelectResult<const N: usize>
where
	[(); N * N]:,
	[(); N + 1]:, {
	/// Reached an unexpanded node: the parent edge index and leaf state need NN evaluation.
	NeedsEval {
		path: Vec<u32>,
		parent: u32,
		edge_idx: usize,
		leaf_state: GameState<N>,
	},
	/// Reached a terminal or already-expanded node: value known, ready to backprop.
	Terminal { path: Vec<u32>, value: f64 },
}

/// How to assign Q to unvisited children during PUCT selection.
///
/// - `MuZero`: unvisited Q = 0.0 (AlphaZero / MuZero default).
/// - `MiniZero`: unvisited Q = mean of visited siblings (MiniZero §III-B cautious prior).
#[derive(Clone, Copy)]
pub(crate) enum PuctVariant {
	MuZero,
	MiniZero,
}

/// Selection phase only — walks the tree from `node_idx` following PUCT/forced action.
/// Returns either a leaf needing NN evaluation, or a terminal with its value.
pub(crate) fn select<const N: usize>(tree: &Tree, node_idx: u32, forced_root_action: Option<usize>, state: &GameState<N>, c_puct: f32, puct: PuctVariant) -> SelectResult<N>
where
	[(); N * N]:,
	[(); N + 1]:, {
	let mut path: Vec<u32> = Vec::new();
	let mut current = node_idx;
	let mut sim_state = state.clone();
	let mut is_root = true;

	//LOOP: embed termination condition
	while let Some(node) = tree.nodes.get(current as usize).filter(|n| !n.edges.is_empty()) {
		let best_edge_idx = if is_root {
			is_root = false;
			match forced_root_action {
				Some(idx) => idx,
				None => select_edge(&tree.nodes, node, c_puct, puct),
			}
		} else {
			select_edge(&tree.nodes, node, c_puct, puct)
		};

		let child = tree.nodes[current as usize].edges[best_edge_idx].child;
		if child == u32::MAX {
			let mv = tree.nodes[current as usize].edges[best_edge_idx].mv;
			sim_state.play(mv).expect("search selected illegal move");
			return SelectResult::NeedsEval {
				path,
				parent: current,
				edge_idx: best_edge_idx,
				leaf_state: sim_state,
			};
		}

		path.push(current);
		let mv = tree.nodes[current as usize].edges[best_edge_idx].mv;
		sim_state.play(mv).expect("search selected illegal move");
		current = child;
	}

	// Terminal node (no edges) or re-visited terminal
	let value = sim_state.outcome().map(|o| outcome_value(o, sim_state.turn)).unwrap_or(0.0);
	SelectResult::Terminal { path, value: value as f64 }
}

/// Expansion + backprop for one pending leaf after batch evaluation.
pub(crate) fn expand_and_backprop<const N: usize>(tree: &mut Tree, path: Vec<u32>, parent: u32, edge_idx: usize, leaf_state: &GameState<N>, eval: Evaluation)
where
	[(); N * N]:,
	[(); N + 1]:, {
	let child_idx = if let Some(outcome) = leaf_state.outcome() {
		tree.expand_terminal(outcome_value(outcome, leaf_state.turn))
	} else {
		tree.expand(eval)
	};
	tree.nodes[parent as usize].edges[edge_idx].child = child_idx;
	backpropagate(tree, &path, tree.nodes[child_idx as usize].total_value);
}

/// One simulation: select -> expand -> backpropagate.
///
/// `forced_root_action`: if `Some(i)`, always take root edge `i` on the first step (Gumbel).
/// If `None`, use PUCT at the root as normal.
pub(crate) fn simulate<const N: usize>(
	tree: &mut Tree,
	node_idx: u32,
	forced_root_action: Option<usize>,
	state: &GameState<N>,
	evaluator: &impl Evaluator<N>,
	c_puct: f32,
	puct: PuctVariant,
) where
	[(); N * N]:,
	[(); N + 1]:, {
	match select(tree, node_idx, forced_root_action, state, c_puct, puct) {
		SelectResult::NeedsEval {
			path,
			parent,
			edge_idx,
			ref leaf_state,
		} => {
			let eval = evaluator.evaluate(leaf_state);
			expand_and_backprop(tree, path, parent, edge_idx, leaf_state, eval);
		}
		SelectResult::Terminal { path, value } => {
			backpropagate(tree, &path, value);
		}
	}
}

fn select_edge(nodes: &[Node], node: &Node, c_puct: f32, puct: PuctVariant) -> usize {
	let parent_visits = node.visit_count;
	let q_unvisited = match puct {
		PuctVariant::MuZero => 0.0f64,
		PuctVariant::MiniZero => {
			let mut n_sigma = 0u32;
			let mut q_sigma = 0.0f64;
			for edge in &node.edges {
				if edge.child != u32::MAX {
					n_sigma += 1;
					q_sigma += -nodes[edge.child as usize].q();
				}
			}
			q_sigma / (n_sigma + 1) as f64
		}
	};
	(0..node.edges.len())
		.max_by(|&a, &b| {
			let sa = edge_uct(nodes, &node.edges[a], parent_visits, q_unvisited, c_puct);
			let sb = edge_uct(nodes, &node.edges[b], parent_visits, q_unvisited, c_puct);
			sa.partial_cmp(&sb).expect("NaN in UCT")
		})
		.expect("edges is non-empty")
}

fn edge_uct(nodes: &[Node], edge: &Edge, parent_visits: u32, q_unvisited: f64, c_puct: f32) -> f64 {
	let (child_q, child_visits) = if edge.child == u32::MAX {
		(q_unvisited, 0)
	} else {
		let child = &nodes[edge.child as usize];
		(-child.q(), child.visit_count)
	};
	// Q + c * P * sqrt(N_parent) / (1 + N_child)
	child_q + c_puct as f64 * edge.prior as f64 * (parent_visits as f64).sqrt() / (1.0 + child_visits as f64)
}

/// Walk back up the path, negating at each level (zero-sum).
pub(crate) fn backpropagate_pub(tree: &mut Tree, path: &[u32], leaf_value: f64) {
	backpropagate(tree, path, leaf_value);
}

fn backpropagate(tree: &mut Tree, path: &[u32], leaf_value: f64) {
	let mut value = leaf_value;
	for &n_idx in path.iter().rev() {
		value = -value;
		let n = &mut tree.nodes[n_idx as usize];
		n.visit_count += 1;
		n.total_value += value;
	}
}

// --- SearchBot trait + VanillaBot ---

impl<E, const N: usize> SearchBot<E, N> for VanillaMcts<E>
where
	E: Evaluator<N> + Send + Sync,
	[(); N * N]:,
	[(); N + 1]:,
{
	fn with_sims(evaluator: E, sims: u32) -> Self {
		Self::new(evaluator, sims)
	}
}

impl<E, const N: usize> Bot<N> for VanillaMcts<E>
where
	E: Evaluator<N> + Send + Sync,
	[(); N * N]:,
	[(); N + 1]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let mut tree = Tree::default();
		let root = tree.expand(self.evaluator.evaluate(game));
		for _ in 0..self.sims {
			simulate(&mut tree, root, None, game, &self.evaluator, self.c_puct, PuctVariant::MuZero);
		}
		tree.nodes[root as usize]
			.edges
			.iter()
			.max_by_key(|e| if e.child == u32::MAX { 0 } else { tree.nodes[e.child as usize].visit_count })
			.expect("root has no edges")
			.mv
	}
}
