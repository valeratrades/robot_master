use std::collections::BTreeMap;

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

/// Vanilla UCT-MCTS bot. Runs `sims` full simulations from the root, picks most-visited child.
pub struct VanillaMcts<E> {
	evaluator: E,
	sims: u32,
	puct_init: f32,
	puct_base: f32,
}
impl<E> VanillaMcts<E> {
	pub fn new(evaluator: E, sims: u32) -> Self {
		Self {
			evaluator,
			sims,
			puct_init: 1.25,
			puct_base: 19652.0,
		}
	}
}

/// `f32` wrapper that is totally ordered and panics on NaN (fail fast).
#[derive(Clone, Copy, PartialEq)]
struct OrdF32(f32);

impl OrdF32 {
	fn new(v: f32) -> Self {
		assert!(!v.is_nan(), "NaN in tree Q-value");
		Self(v)
	}
}

impl Eq for OrdF32 {}

impl PartialOrd for OrdF32 {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for OrdF32 {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.0.total_cmp(&other.0)
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
	/// Live sorted multiset of Q values across all nodes — mirrors MiniZero's `tree_value_bound_`.
	/// Key = Q value, value = ref count. Min/max are always exact (never stale).
	value_bound: BTreeMap<OrdF32, u32>,
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
			value_bound: BTreeMap::new(),
		}
	}

	/// Exact Q bounds from the live sorted multiset.
	/// Returns `(NaN, NaN)` when fewer than 2 distinct values exist (degenerate case).
	/// Mirrors MiniZero `getNormalizedMean`'s early-return condition.
	pub(crate) fn q_bounds(&self) -> (f32, f32) {
		if self.value_bound.len() < 2 {
			return (f32::NAN, f32::NAN);
		}
		(self.value_bound.first_key_value().unwrap().0.0, self.value_bound.last_key_value().unwrap().0.0)
	}

	/// Update the live Q-value multiset after a node's Q changes from `old_q` to `new_q`.
	/// Mirrors MiniZero `updateTreeValueBound`.
	pub(crate) fn update_value_bound(&mut self, old_q: f32, new_q: f32) {
		// Remove old entry
		let old_key = OrdF32::new(old_q);
		let count = self.value_bound.get_mut(&old_key).expect("old_q must be present in value_bound");
		if *count <= 1 {
			self.value_bound.remove(&old_key);
		} else {
			*count -= 1;
		}
		// Insert new entry
		*self.value_bound.entry(OrdF32::new(new_q)).or_insert(0) += 1;
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
			.expect("root must have at least one edge")
	}

	/// Normalized Q for root edge `action_idx` — mirrors MiniZero `getNormalizedMean`.
	/// When bounds are degenerate (< 2 distinct values), returns 1.0 per MiniZero convention.
	pub(crate) fn root_q_normalized(&self, action_idx: usize) -> f32 {
		let (q_min, q_max) = self.q_bounds();
		normalize_q(self.root_q_raw(action_idx), q_min, q_max)
	}
}

impl Default for Tree {
	fn default() -> Self {
		Self {
			nodes: Vec::new(),
			value_bound: BTreeMap::new(),
		}
	}
}

/// Normalize a raw Q value to [-1,1] using the live tree min/max bounds.
/// Mirrors MiniZero `getNormalizedMean` (mcts.cpp:44-48).
/// When `q_min` is NaN (< 2 distinct values in the tree), returns 1.0.
pub(crate) fn normalize_q(raw: f32, q_min: f32, q_max: f32) -> f32 {
	if q_min.is_nan() {
		return 1.0;
	}
	let x = ((raw - q_min) / (q_max - q_min)).clamp(0.0, 1.0);
	2.0 * x - 1.0
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
pub(crate) fn select<const N: usize>(
	tree: &Tree,
	node_idx: u32,
	forced_root_action: Option<usize>,
	state: &GameState<N>,
	puct_init: f32,
	puct_base: f32,
	puct: PuctVariant,
) -> SelectResult<N>
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
				None => select_edge(tree, node, puct_init, puct_base, puct),
			}
		} else {
			select_edge(tree, node, puct_init, puct_base, puct)
		};

		path.push(current);
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
		let mv = tree.nodes[current as usize].edges[best_edge_idx].mv;
		sim_state.play(mv).expect("search selected illegal move");
		current = child;
	}

	// Terminal node (no edges) — sim_state must have a known outcome here.
	let value = outcome_value(sim_state.outcome().expect("node with no edges must be terminal"), sim_state.turn);
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
	puct_init: f32,
	puct_base: f32,
	puct: PuctVariant,
) where
	[(); N * N]:,
	[(); N + 1]:, {
	match select(tree, node_idx, forced_root_action, state, puct_init, puct_base, puct) {
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

fn select_edge(tree: &Tree, node: &Node, puct_init: f32, puct_base: f32, puct: PuctVariant) -> usize {
	let parent_visits = node.visit_count;
	let (q_min, q_max) = tree.q_bounds();
	let q_unvisited = match puct {
		PuctVariant::MuZero => 0.0f64,
		PuctVariant::MiniZero => {
			// MiniZero calculateInitQValue (mcts.cpp:200-216, board-game branch):
			//   (Σ getNormalizedMean(child) - 1) / (n_visited + 1)
			// This is a pessimistic estimate: mean of visited children minus one "virtual loss".
			let mut n_visited = 0u32;
			let mut q_sum = 0.0f64;
			for edge in &node.edges {
				if edge.child != u32::MAX {
					n_visited += 1;
					let raw = -tree.nodes[edge.child as usize].q() as f32;
					q_sum += normalize_q(raw, q_min, q_max) as f64;
				}
			}
			(q_sum - 1.0) / (n_visited + 1) as f64
		}
	};
	(0..node.edges.len())
		.max_by(|&a, &b| {
			let sa = edge_uct(tree, &node.edges[a], parent_visits, q_unvisited, puct_init, puct_base, q_min, q_max);
			let sb = edge_uct(tree, &node.edges[b], parent_visits, q_unvisited, puct_init, puct_base, q_min, q_max);
			sa.partial_cmp(&sb).expect("NaN in UCT")
		})
		.expect("edges is non-empty")
}

fn edge_uct(tree: &Tree, edge: &Edge, parent_visits: u32, q_unvisited: f64, puct_init: f32, puct_base: f32, q_min: f32, q_max: f32) -> f64 {
	let (child_q, child_visits) = if edge.child == u32::MAX {
		(q_unvisited, 0)
	} else {
		let child = &tree.nodes[edge.child as usize];
		// MiniZero uses getNormalizedMean for visited children in PUCT.
		let raw = -child.q() as f32;
		(normalize_q(raw, q_min, q_max) as f64, child.visit_count)
	};
	// MiniZero getNormalizedPUCTScore (mcts.cpp:57-58):
	//   puct_bias = puct_init + log((1 + N + puct_base) / puct_base)
	//   value_u = puct_bias * P * sqrt(N_parent) / (1 + N_child)
	let total_sim = parent_visits.saturating_sub(1) as f64;
	let puct_bias = puct_init as f64 + ((1.0 + total_sim + puct_base as f64) / puct_base as f64).ln();
	child_q + puct_bias * edge.prior as f64 * total_sim.sqrt() / (1.0 + child_visits as f64)
}

/// Walk back up the path, negating at each level (zero-sum).
pub(crate) fn backpropagate_pub(tree: &mut Tree, path: &[u32], leaf_value: f64) {
	backpropagate(tree, path, leaf_value);
}

fn backpropagate(tree: &mut Tree, path: &[u32], leaf_value: f64) {
	let mut value = leaf_value;
	for &n_idx in path.iter().rev() {
		value = -value;
		// Capture Q before the update so we can remove the old entry from the multiset.
		// MiniZero mcts.cpp:174-176: old_mean captured, node updated, then updateTreeValueBound.
		let old_q = tree.nodes[n_idx as usize].q() as f32;
		{
			let n = &mut tree.nodes[n_idx as usize];
			n.visit_count += 1;
			n.total_value += value;
		}
		let new_q = tree.nodes[n_idx as usize].q() as f32;
		// First visit: old_q is from the initial total_value/1 set at node creation.
		// That initial Q was never inserted into value_bound, so we skip removal on visit_count==1.
		// After the increment above, visit_count >= 2 means the old_q was tracked.
		if tree.nodes[n_idx as usize].visit_count > 2 {
			tree.update_value_bound(old_q, new_q);
		} else {
			// Second visit: old_q (from visit_count==1) was never in the map; just insert new.
			*tree.value_bound.entry(OrdF32::new(new_q)).or_insert(0) += 1;
		}
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
			simulate(&mut tree, root, None, game, &self.evaluator, self.puct_init, self.puct_base, PuctVariant::MuZero);
		}
		tree.nodes[root as usize]
			.edges
			.iter()
			.max_by_key(|e| if e.child == u32::MAX { 0 } else { tree.nodes[e.child as usize].visit_count })
			.expect("root has no edges")
			.mv
	}
}
