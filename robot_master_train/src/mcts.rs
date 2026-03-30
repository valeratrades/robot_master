use board_game::board::{Board as _, Outcome};
use robot_master_arena::player::Bot;
use robot_master_core::game::{GameState, Move};
use ustr::{Ustr, ustr};

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

pub struct MctsConfig {
	pub simulations: u32,
	pub c_puct: f32,
}
/// Run MCTS from `state`, return the best move.
pub fn search<const N: usize>(state: &GameState<N>, evaluator: &impl Evaluator<N>, config: &MctsConfig) -> Move
where
	[(); N * N]:, {
	let mut tree = Tree::new();
	let root = tree.expand(evaluator.evaluate(state));

	for _ in 0..config.simulations {
		simulate(&mut tree, root, state, evaluator, config.c_puct);
	}

	// Pick the most-visited child.
	let root_node = &tree.nodes[root as usize];
	root_node
		.edges
		.iter()
		.max_by_key(|e| if e.child == u32::MAX { 0 } else { tree.nodes[e.child as usize].visit_count })
		.expect("root has no edges")
		.mv
}
/// MCTS-based bot: wraps `search` and implements `Bot<N>`.
pub struct MctsBot<E> {
	evaluator: E,
	config: MctsConfig,
}
impl<E> MctsBot<E> {
	pub fn new(evaluator: E, config: MctsConfig) -> Self {
		Self { evaluator, config }
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

// --- MCTS Tree ---

struct Edge {
	mv: Move,
	prior: f32,
	/// Index into `Tree::nodes`, or `u32::MAX` if unexpanded.
	child: u32,
}

struct Node {
	/// Total value accumulated through this node (from the perspective of the player who moved *to* this node).
	total_value: f64,
	visit_count: u32,
	edges: Vec<Edge>,
}

impl Node {
	fn q(&self) -> f64 {
		if self.visit_count == 0 { 0.0 } else { self.total_value / self.visit_count as f64 }
	}
}

struct Tree {
	nodes: Vec<Node>,
}

impl Tree {
	fn new() -> Self {
		Self { nodes: Vec::new() }
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
}

impl Default for MctsConfig {
	fn default() -> Self {
		Self { simulations: 800, c_puct: 1.41 }
	}
}

/// One simulation: select -> expand -> backpropagate.
fn simulate<const N: usize>(tree: &mut Tree, node_idx: u32, state: &GameState<N>, evaluator: &impl Evaluator<N>, c_puct: f32)
where
	[(); N * N]:, {
	let mut path: Vec<u32> = Vec::new(); // node indices along the path
	let mut current = node_idx;
	let mut sim_state = state.clone();

	//LOOP: embed termination condition
	while let Some(node) = tree.nodes.get(current as usize).filter(|n| !n.edges.is_empty()) {
		let parent_visits = node.visit_count;
		let best_edge_idx = select_edge(&tree.nodes, node, parent_visits, c_puct);

		path.push(current);
		let mv = tree.nodes[current as usize].edges[best_edge_idx].mv;
		sim_state.play(mv).expect("MCTS selected illegal move");

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

fn select_edge(nodes: &[Node], node: &Node, parent_visits: u32, c_puct: f32) -> usize {
	(0..node.edges.len())
		.max_by(|&a, &b| {
			let sa = edge_uct(nodes, &node.edges[a], parent_visits, c_puct);
			let sb = edge_uct(nodes, &node.edges[b], parent_visits, c_puct);
			sa.partial_cmp(&sb).expect("NaN in UCT")
		})
		.expect("edges is non-empty")
}

fn edge_uct(nodes: &[Node], edge: &Edge, parent_visits: u32, c_puct: f32) -> f64 {
	let (child_q, child_visits) = if edge.child == u32::MAX {
		(0.0, 0)
	} else {
		let child = &nodes[edge.child as usize];
		(-child.q(), child.visit_count)
	};
	// Q + c * P * sqrt(N_parent) / (1 + N_child)
	child_q + c_puct as f64 * edge.prior as f64 * (parent_visits as f64).sqrt() / (1.0 + child_visits as f64)
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

impl<E, const N: usize> Bot<N> for MctsBot<E>
where
	E: Evaluator<N> + Send + Sync,
	[(); N * N]:,
{
	fn id(&self) -> Ustr {
		ustr("mcts")
	}

	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		search(game, &self.evaluator, &self.config)
	}
}

#[cfg(test)]
mod tests {
	use board_game::board::Board as _;
	use rand::{SeedableRng, rngs::SmallRng};
	use robot_master_arena::algos::rollout::RolloutPlayer;
	use robot_master_core::game::{GameConfig, GameState};

	use super::*;

	fn state5() -> GameState<5> {
		let mut rng = SmallRng::seed_from_u64(42);
		GameState::new(GameConfig::default(), &mut rng)
	}

	#[test]
	fn mcts_returns_legal_move() {
		let state = state5();
		let evaluator = RolloutEval::new(RolloutPlayer);
		let config = MctsConfig { simulations: 50, c_puct: 1.41 };
		let mv = search(&state, &evaluator, &config);

		assert!(state.valid_moves().any(|m| m == mv), "MCTS returned illegal move: {mv}");
	}

	#[test]
	fn mcts_bot_plays_full_game() {
		let mut state = state5();
		let mut bot = MctsBot::new(RolloutEval::new(RolloutPlayer), MctsConfig { simulations: 20, c_puct: 1.41 });

		while state.outcome().is_none() {
			let mv = bot.choose_move(&state);
			state.play(mv).expect("illegal move");
		}

		assert!(state.outcome().is_some());
	}
}
