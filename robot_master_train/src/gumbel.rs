/// Gumbel AlphaZero search.
///
/// Replaces the standard MCTS + Dirichlet noise + temperature sampling used in AlphaGo Zero
/// with Sequential Halving guided by Gumbel noise. Key properties:
///
/// - Guarantees policy improvement: E[q(A_{n+1})] >= E_{a~π}[q(a)]
/// - Works reliably with n=2..32 simulations (vs 400+ for standard MCTS)
/// - No Dirichlet noise, no temperature schedule - exploration comes from Gumbel sampling
/// - Policy target = softmax(logits + σ(completedQ)), not visit counts
///
/// Reference: "Policy Improvement by Planning with Gumbel", Danihelka et al., ICLR 2022.
/// See docs/references/gumbel_alphazero.md for full details.
use rand::Rng;
use rand_distr::{Distribution as _, Gumbel};
use robot_master_arena::player::Bot;
use robot_master_core::game::{GameState, Move};

use crate::mcts::{Evaluation, Evaluator, PuctVariant, SearchBot, SelectResult, Tree, backpropagate_pub, expand_and_backprop, normalize_q, select, simulate};

#[derive(Clone)]
pub struct GumbelConfig {
	/// Total simulation budget per move.
	pub n_simulations: u32,
	/// Number of root actions sampled without replacement (m ≤ n_simulations).
	/// Default: min(n_simulations, 16).
	pub m_actions: u32,
	/// c_visit in σ(q) = (c_visit + max_N) * c_scale * q. Paper default: 50.
	pub c_visit: f32,
	/// c_scale in σ. MiniZero default: 1.0 (with normalized Q-values in [-1, 1]).
	pub c_scale: f32,
	/// puct_init for non-root PUCT selection. MiniZero/AlphaZero default: 1.25.
	/// puct_bias = puct_init + log((1 + N + puct_base) / puct_base)
	pub puct_init: f32,
	/// puct_base for non-root PUCT selection. MiniZero/AlphaZero default: 19652.
	pub puct_base: f32,
}

impl Default for GumbelConfig {
	fn default() -> Self {
		let n = 16;
		Self {
			n_simulations: n,
			m_actions: n.min(16),
			c_visit: 50.0,
			c_scale: 1.0,
			puct_init: 1.25,
			puct_base: 19652.0,
		}
	}
}

/// Output of one Gumbel search: the move to play and the improved policy target.
pub struct GumbelResult {
	/// The action selected by Sequential Halving to play.
	pub mv: Move,
	/// Improved policy π' = softmax(logits + σ(completedQ)), indexed by action.
	/// Same length and ordering as the legal moves in the root evaluation.
	pub policy_target: Vec<(Move, f32)>,
	/// Backed-up MCTS mean at the root from the current player's perspective.
	/// Used as the value training target (self-consistent bootstrap signal).
	pub root_value_mean: f32,
}

/// Run Gumbel AlphaZero search from `state`.
///
/// Returns the move to play and the completed-Q policy target for training.
pub fn gumbel_search<const N: usize, E, R>(state: &GameState<N>, evaluator: &E, config: &GumbelConfig, rng: &mut R) -> GumbelResult
where
	E: Evaluator<N>,
	R: Rng,
	[(); N * N]:,
	[(); N + 1]:, {
	let root_eval = evaluator.evaluate(state);
	let k = root_eval.policy.len();
	assert!(k > 0, "no legal moves at root");

	// Normalize priors once
	let prior_sum: f32 = root_eval.policy.iter().map(|(_, p)| p).sum();
	let priors: Vec<f32> = root_eval.policy.iter().map(|(_, p)| p / prior_sum).collect();
	let moves: Vec<Move> = root_eval.policy.iter().map(|(mv, _)| *mv).collect();
	let root_value = root_eval.value; // v̂_π from the value head

	// Log-probabilities (logits = log π) for Gumbel scoring
	let logits: Vec<f32> = priors.iter().map(|&p| p.max(1e-8).ln()).collect();

	// Step 1: sample k Gumbel(0,1) variables once - reused throughout
	let gumbel_dist = Gumbel::new(0.0f32, 1.0).expect("valid Gumbel params");
	let g: Vec<f32> = (0..k).map(|_| gumbel_dist.sample(rng)).collect();

	// Step 2: select top-m actions without replacement by g + logits
	let m = (config.m_actions as usize).min(k).min(config.n_simulations as usize).max(1);
	let gumbel_scores: Vec<f32> = (0..k).map(|i| g[i] + logits[i]).collect();
	let top_m: Vec<usize> = argtop_m(&gumbel_scores, m);

	// Step 3: Sequential Halving - allocate n simulations over top_m
	let mut tree = Tree::new_with_root(root_value, &moves, &priors);
	let root_idx = 0u32;

	let n = config.n_simulations as usize;
	let phases = (m as f32).log2().ceil() as usize;
	let phases = phases.max(1);

	let mut survivors: Vec<usize> = top_m; // indices into moves[]
	let mut sims_used = 0usize;

	for phase in 0..phases {
		let remaining_phases = phases - phase;
		let sims_per_action = (n.saturating_sub(sims_used)) / (remaining_phases * survivors.len()).max(1);
		let sims_per_action = sims_per_action.max(1);

		run_phase_batched(
			&mut tree, root_idx, state, evaluator, config.puct_init, config.puct_base, &survivors, sims_per_action, n, &mut sims_used,
		);

		if survivors.len() <= 1 {
			break;
		}

		// Rank survivors by g(a) + logits(a) + σ(q̂(a)), drop bottom half
		let max_visits = tree.max_root_visits();
		survivors.sort_unstable_by(|&a, &b| {
			let sa = gumbel_scores[a] + sigma(tree.root_q_normalized(a), max_visits, config);
			let sb = gumbel_scores[b] + sigma(tree.root_q_normalized(b), max_visits, config);
			sb.partial_cmp(&sa).expect("NaN in Gumbel score")
		});
		survivors.truncate(survivors.len().div_ceil(2));
	}

	// Spend any remaining budget on the last survivor(s)
	while sims_used < n {
		simulate(
			&mut tree,
			root_idx,
			Some(survivors[0]),
			state,
			evaluator,
			config.puct_init,
			config.puct_base,
			PuctVariant::MiniZero,
		);
		sims_used += 1;
	}

	// Step 4: select A_{n+1} - argmax of g + logits + σ(q̂) among survivors
	let max_visits = tree.max_root_visits();
	let best_idx = *survivors
		.iter()
		.max_by(|&&a, &&b| {
			let sa = gumbel_scores[a] + sigma(tree.root_q_normalized(a), max_visits, config);
			let sb = gumbel_scores[b] + sigma(tree.root_q_normalized(b), max_visits, config);
			sa.partial_cmp(&sb).expect("NaN in Gumbel score")
		})
		.expect("survivors non-empty");

	// Step 5: compute policy target π' = softmax(logits + σ(completedQ))
	// v_mix and root_q_normalized are both already in [-1,1] (normalized space), matching
	// MiniZero's getMCTSPolicy which builds q_sum from getNormalizedMean.
	let v_mix = compute_v_mix(root_value, &priors, &tree, k, config.n_simulations);
	let completed_q: Vec<f32> = (0..k).map(|i| if tree.root_visited(i) { tree.root_q_normalized(i) } else { v_mix }).collect();

	let max_visits_f = max_visits as f32;
	let improved_logits: Vec<f32> = (0..k).map(|i| logits[i] + (config.c_visit + max_visits_f) * config.c_scale * completed_q[i]).collect();
	let policy_target = softmax_to_moves(&moves, &improved_logits);

	GumbelResult {
		mv: moves[best_idx],
		policy_target,
		root_value_mean: tree.nodes[0].q() as f32,
	}
}

// ---------------------------------------------------------------------------
// Resumable Gumbel search - for vectorized (GPU-batched) self-play
// ---------------------------------------------------------------------------

/// A Gumbel search that can be paused at each NN evaluation point and resumed
/// after receiving a batch of evaluations from outside.
///
/// Lifecycle:
///   1. `GumbelSearch::new(state, root_eval, config, gumbel_scores, moves, priors)`
///   2. Loop: `collect_pending_selections()` → caller batches evals → `apply_evals(evals)`
///   3. Until `is_done()`, then call `finish()` to get `GumbelResult`.
///
/// Invariant: between `collect_pending_selections` and `apply_evals` the tree
/// must not be mutated. After `apply_evals` returns, the state machine has
/// advanced (phase updated, survivors trimmed if needed) and is ready for the
/// next `collect_pending_selections` call.
pub struct GumbelSearch<const N: usize>
where
	[(); N * N]:,
	[(); N + 1]:, {
	tree: Tree,
	root_state: GameState<N>,
	moves: Vec<Move>,
	priors: Vec<f32>,
	logits: Vec<f32>,
	gumbel_scores: Vec<f32>,
	root_value: f32,
	config: GumbelConfig,
	/// Total sim budget.
	n: usize,
	/// Phases of Sequential Halving.
	phases: usize,
	/// Current phase index (>= phases means "drain remaining budget" tail).
	phase: usize,
	survivors: Vec<usize>,
	sims_used: usize,
	/// Selections from the last `collect_pending_selections` call, waiting for evals.
	pending: Vec<PendingLeaf<N>>,
	/// Whether the phase just dispatched needs survivor halving after evals are applied.
	phase_needs_halving: bool,
	done: bool,
}
impl<const N: usize> GumbelSearch<N>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	/// Start a new search. `root_eval` is the NN evaluation of `state` - the
	/// caller is responsible for batching root evaluations across games.
	pub fn new(state: &GameState<N>, root_eval: Evaluation, config: GumbelConfig, gumbel_scores: Vec<f32>, moves: Vec<Move>, priors: Vec<f32>) -> Self {
		let root_value = root_eval.value;
		let logits: Vec<f32> = priors.iter().map(|&p| p.max(1e-8).ln()).collect();
		let n = config.n_simulations as usize;
		let m = (config.m_actions as usize).min(moves.len()).min(n).max(1);
		let phases = (m as f32).log2().ceil() as usize;
		let phases = phases.max(1);

		let top_m = argtop_m(&gumbel_scores, m);
		let tree = Tree::new_with_root(root_value, &moves, &priors);

		Self {
			tree,
			root_state: state.clone(),
			moves,
			priors,
			logits,
			gumbel_scores,
			root_value,
			config,
			n,
			phases,
			phase: 0,
			survivors: top_m,
			sims_used: 0,
			pending: Vec::default(),
			phase_needs_halving: false,
			done: false,
		}
	}

	pub fn is_done(&self) -> bool {
		self.done
	}

	/// Run selection for the current batch step. Returns the states that need
	/// NN evaluation. The caller must call `apply_evals` with exactly this many
	/// evaluations before calling `collect_pending_selections` again.
	///
	/// If all remaining sims hit terminals or already-expanded nodes (no NN
	/// needed), this advances the state machine internally and may set `done`.
	pub fn collect_pending_selections(&mut self) -> &[PendingLeaf<N>] {
		assert!(self.pending.is_empty(), "apply_evals must be called before collect_pending_selections");

		let mut pending_edges: std::collections::HashSet<(u32, usize)> = std::collections::HashSet::default();

		let (action_indices, sims_per_action) = if self.phase < self.phases {
			let remaining_phases = self.phases - self.phase;
			let spa = (self.n.saturating_sub(self.sims_used)) / (remaining_phases * self.survivors.len()).max(1);
			let spa = spa.max(1);
			(self.survivors.clone(), spa)
		} else {
			// Drain: remaining budget goes to survivors[0]
			(vec![self.survivors[0]], self.n.saturating_sub(self.sims_used))
		};

		for &action_idx in &action_indices {
			for _ in 0..sims_per_action {
				if self.sims_used >= self.n {
					break;
				}
				self.sims_used += 1;

				match select(
					&self.tree,
					0,
					Some(action_idx),
					&self.root_state,
					self.config.puct_init,
					self.config.puct_base,
					PuctVariant::MiniZero,
				) {
					SelectResult::Terminal { path, value } => {
						backpropagate_pub(&mut self.tree, &path, value);
					}
					SelectResult::NeedsEval { path, parent, edge_idx, leaf_state } => {
						if pending_edges.insert((parent, edge_idx)) {
							self.pending.push(PendingLeaf { path, parent, edge_idx, leaf_state });
						}
						// Duplicate edge in same batch: sim budget consumed but no tree update.
						// Rare at typical sim counts; preferable to a single-sample GPU call.
					}
				}
			}
		}

		// Mark that this phase's sims are dispatched; halving happens in apply_evals
		// once the Q-values are actually updated by backpropagation.
		if self.phase < self.phases {
			self.phase += 1;
			self.phase_needs_halving = self.survivors.len() > 1;
		}

		if self.pending.is_empty() {
			// All sims resolved as terminals - no evals needed, so halve now.
			self.halve_survivors_if_needed();
			if self.sims_used >= self.n {
				self.done = true;
			}
		}

		&self.pending
	}

	/// Halve survivors using up-to-date Q values. Must only be called after the
	/// current phase's backpropagation is complete.
	fn halve_survivors_if_needed(&mut self) {
		if !self.phase_needs_halving {
			return;
		}
		self.phase_needs_halving = false;
		let max_visits = self.tree.max_root_visits();
		self.survivors.sort_unstable_by(|&a, &b| {
			let sa = self.gumbel_scores[a] + sigma(self.tree.root_q_normalized(a), max_visits, &self.config);
			let sb = self.gumbel_scores[b] + sigma(self.tree.root_q_normalized(b), max_visits, &self.config);
			sb.partial_cmp(&sa).expect("NaN in Gumbel score")
		});
		self.survivors.truncate(self.survivors.len().div_ceil(2));
	}

	/// Expand and backpropagate results from the last `collect_pending_selections`.
	/// `evals` must have the same length as the slice returned by the last call.
	pub fn apply_evals(&mut self, evals: Vec<Evaluation>) {
		assert_eq!(evals.len(), self.pending.len(), "eval count must match pending leaf count");

		// Drain pending so we can consume both vecs together
		let pending = std::mem::take(&mut self.pending);
		for (p, eval) in pending.into_iter().zip(evals) {
			expand_and_backprop(&mut self.tree, p.path, p.parent, p.edge_idx, &p.leaf_state, eval);
		}

		// Halve survivors now that Q-values are updated from this phase's backprop.
		self.halve_survivors_if_needed();

		if self.sims_used >= self.n {
			self.done = true;
		}
	}

	/// Consume the search and produce the final result. Must only be called when `is_done()`.
	pub fn finish(self) -> GumbelResult {
		assert!(self.done, "finish called before search is done");
		let k = self.moves.len();
		let max_visits = self.tree.max_root_visits();

		let best_idx = *self
			.survivors
			.iter()
			.max_by(|&&a, &&b| {
				let sa = self.gumbel_scores[a] + sigma(self.tree.root_q_normalized(a), max_visits, &self.config);
				let sb = self.gumbel_scores[b] + sigma(self.tree.root_q_normalized(b), max_visits, &self.config);
				sa.partial_cmp(&sb).expect("NaN in Gumbel score")
			})
			.expect("survivors non-empty");

		let v_mix = compute_v_mix(self.root_value, &self.priors, &self.tree, k, self.config.n_simulations);
		let completed_q: Vec<f32> = (0..k).map(|i| if self.tree.root_visited(i) { self.tree.root_q_normalized(i) } else { v_mix }).collect();

		let max_visits_f = max_visits as f32;
		let improved_logits: Vec<f32> = (0..k)
			.map(|i| self.logits[i] + (self.config.c_visit + max_visits_f) * self.config.c_scale * completed_q[i])
			.collect();
		let policy_target = softmax_to_moves(&self.moves, &improved_logits);

		GumbelResult {
			mv: self.moves[best_idx],
			policy_target,
			root_value_mean: self.tree.nodes[0].q() as f32,
		}
	}
}

pub struct PendingLeaf<const N: usize>
where
	[(); N * N]:,
	[(); N + 1]:, {
	pub(crate) path: Vec<u32>,
	pub(crate) parent: u32,
	pub(crate) edge_idx: usize,
	pub(crate) leaf_state: GameState<N>,
}

/// Sample Gumbel scores and normalize priors for a state - shared setup for both
/// blocking `gumbel_search` and the resumable `GumbelSearch`.
pub fn gumbel_setup<const N: usize, R: Rng>(root_eval: &Evaluation, rng: &mut R) -> (Vec<f32>, Vec<Move>, Vec<f32>)
where
	[(); N * N]:,
	[(); N + 1]:, {
	let prior_sum: f32 = root_eval.policy.iter().map(|(_, p)| p).sum();
	let priors: Vec<f32> = root_eval.policy.iter().map(|(_, p)| p / prior_sum).collect();
	let moves: Vec<Move> = root_eval.policy.iter().map(|(mv, _)| *mv).collect();
	let k = moves.len();
	let logits: Vec<f32> = priors.iter().map(|&p| p.max(1e-8).ln()).collect();
	let gumbel_dist = Gumbel::new(0.0f32, 1.0).expect("valid Gumbel params");
	let g: Vec<f32> = (0..k).map(|_| gumbel_dist.sample(rng)).collect();
	let gumbel_scores: Vec<f32> = (0..k).map(|i| g[i] + logits[i]).collect();
	(gumbel_scores, moves, priors)
}

/// Gumbel-based bot: wraps `gumbel_search` and implements `Bot<N>`.
pub struct GumbelMcts<E> {
	evaluator: E,
	config: GumbelConfig,
}

impl<E> GumbelMcts<E> {
	pub fn new(evaluator: E, config: GumbelConfig) -> Self {
		Self { evaluator, config }
	}
}

/// Per-move search health stats returned by [`search_diag`].
pub struct SearchDiag {
	/// Value predicted by the evaluator for this position (current player's perspective).
	pub root_value: f32,
	/// Width of the raw Q range across all completed actions (visited + v_mix for unvisited).
	/// Near zero means the value head isn't differentiating positions → no policy signal.
	pub q_range_raw: f32,
	/// Variance of completed_q (normalized to [0,1]).
	/// Near zero means the policy target is flat → the training label is nearly uniform.
	pub completed_q_variance: f32,
	/// completed_q of the action selected by the search.
	pub selected_cq: f32,
	/// Mean completed_q across all actions (always ~0.5 when flat).
	pub mean_cq: f32,
}
/// Run one Gumbel search and return diagnostic stats for the policy-signal health.
/// Uses only `m` simulations (one per top-m action) to keep it cheap.
pub fn search_diag<const N: usize, E, R>(state: &GameState<N>, evaluator: &E, config: &GumbelConfig, rng: &mut R) -> SearchDiag
where
	E: Evaluator<N>,
	R: Rng,
	[(); N * N]:,
	[(); N + 1]:, {
	let root_eval = evaluator.evaluate(state);
	let k = root_eval.policy.len();
	let root_value = root_eval.value;

	let (gumbel_scores, moves, priors) = gumbel_setup::<N, _>(&root_eval, rng);
	let m = (config.m_actions as usize).min(k).min(config.n_simulations as usize).max(1);
	let top_m = argtop_m(&gumbel_scores, m);

	let mut tree = Tree::new_with_root(root_value, &moves, &priors);
	for &action_idx in &top_m {
		simulate(&mut tree, 0, Some(action_idx), state, evaluator, config.puct_init, config.puct_base, PuctVariant::MiniZero);
	}

	let v_mix = compute_v_mix(root_value, &priors, &tree, k, config.n_simulations);
	let (q_min, q_max) = tree.q_bounds();
	let q_range_raw = if q_min.is_nan() { 0.0 } else { q_max - q_min };

	let completed_q: Vec<f32> = (0..k).map(|i| if tree.root_visited(i) { tree.root_q_normalized(i) } else { v_mix }).collect();
	let mean_cq = completed_q.iter().sum::<f32>() / k as f32;
	let completed_q_variance = completed_q.iter().map(|&q| (q - mean_cq).powi(2)).sum::<f32>() / k as f32;

	// Best action = argmax gumbel_scores + sigma(root_q) among top_m (one phase only)
	let max_visits = tree.max_root_visits();
	let best_idx = *top_m
		.iter()
		.max_by(|&&a, &&b| {
			let sa = gumbel_scores[a] + sigma(tree.root_q_normalized(a), max_visits, config);
			let sb = gumbel_scores[b] + sigma(tree.root_q_normalized(b), max_visits, config);
			sa.partial_cmp(&sb).expect("NaN")
		})
		.expect("top_m non-empty");
	let selected_cq = completed_q[best_idx];

	SearchDiag {
		root_value,
		q_range_raw,
		completed_q_variance,
		selected_cq,
		mean_cq,
	}
}
/// Run one phase of Sequential Halving using batched leaf evaluation.
///
/// Within a phase, all simulations that need NN evaluation are batched together.
/// The trick: each forced root action leads to a distinct child of the root that is
/// unexpanded at first visit. Once expanded, deeper sims may hit already-expanded
/// nodes and fall through to terminal/re-visit - those are handled individually.
///
/// Because each `action_idx` in `survivors` is a distinct root edge, multiple sims
/// for the same action within a phase can share the same evaluation result for the
/// first expansion (the direct child). Subsequent sims on an already-expanded action
/// descend deeper into its subtree - those may or may not hit new leaves.
///
/// Strategy: collect all `(action_idx, sim)` selections that return NeedsEval into a
/// batch, evaluate once, then expand+backprop. Sims that hit terminals are handled
/// inline without NN calls.
fn run_phase_batched<const N: usize, E>(
	tree: &mut Tree,
	root_idx: u32,
	state: &GameState<N>,
	evaluator: &E,
	puct_init: f32,
	puct_base: f32,
	survivors: &[usize],
	sims_per_action: usize,
	budget: usize,
	sims_used: &mut usize,
) where
	E: Evaluator<N>,
	[(); N * N]:,
	[(); N + 1]:, {
	struct Pending<const M: usize>
	where
		[(); M * M]:,
		[(); M + 1]:, {
		path: Vec<u32>,
		parent: u32,
		edge_idx: usize,
		leaf_state: GameState<M>,
	}

	// Collect all sims that need NN evaluation. Sims that hit already-expanded or
	// terminal nodes are handled eagerly via `simulate` (no NN needed).
	//
	// We run selection against the *current* tree state, which means each sim sees
	// the expansions from previous iterations/phases - this is correct. Within a
	// single phase batch, we collect leaves without yet expanding them, so subsequent
	// selections in the same batch may select the same unexpanded edge. We deduplicate
	// by (parent, edge_idx): if an edge is already in the pending list we fall back to
	// simulate() for that sim (it will re-select from the same node using PUCT).
	let mut pending: Vec<Pending<N>> = Vec::default();
	let mut pending_edges: std::collections::HashSet<(u32, usize)> = std::collections::HashSet::default();

	for &action_idx in survivors {
		for _ in 0..sims_per_action {
			if *sims_used >= budget {
				break;
			}
			*sims_used += 1;

			match select(tree, root_idx, Some(action_idx), state, puct_init, puct_base, PuctVariant::MiniZero) {
				SelectResult::Terminal { path, value } => {
					crate::mcts::backpropagate_pub(tree, &path, value);
				}
				SelectResult::NeedsEval { path, parent, edge_idx, leaf_state } => {
					if pending_edges.insert((parent, edge_idx)) {
						pending.push(Pending { path, parent, edge_idx, leaf_state });
					} else {
						// Another sim in this batch already claimed this leaf; fall back to
						// simulate() which will navigate past the (still-unexpanded) edge
						// via PUCT to find a different leaf or terminal.
						*sims_used -= 1; // undo: simulate counts its own sim
						simulate(tree, root_idx, Some(action_idx), state, evaluator, puct_init, puct_base, PuctVariant::MiniZero);
						*sims_used += 1;
					}
				}
			}
		}
	}

	if pending.is_empty() {
		return;
	}

	// Single batched NN call for all pending leaves
	let leaf_states: Vec<GameState<N>> = pending.iter().map(|p| p.leaf_state.clone()).collect();
	let evals = evaluator.evaluate_batch(&leaf_states);

	for (p, eval) in pending.into_iter().zip(evals) {
		expand_and_backprop(tree, p.path, p.parent, p.edge_idx, &p.leaf_state, eval);
	}
}

// --- helpers ---

/// Return indices of the top-m elements of scores (descending), without replacement.
fn argtop_m(scores: &[f32], m: usize) -> Vec<usize> {
	let mut indexed: Vec<(usize, f32)> = scores.iter().copied().enumerate().collect();
	// partial sort: only need top m
	indexed.select_nth_unstable_by(m.saturating_sub(1), |a, b| b.1.partial_cmp(&a.1).expect("NaN"));
	indexed[..m].iter().map(|&(i, _)| i).collect()
}

/// σ(q̂) = (c_visit + max_N) * c_scale * q_normalized.
fn sigma(q_normalized: f32, max_visits: u32, config: &GumbelConfig) -> f32 {
	(config.c_visit + max_visits as f32) * config.c_scale * q_normalized
}

/// v_mix: interpolation between v̂_π and prior-weighted sum of observed normalized Q-values.
/// All quantities are in normalized [-1,1] space, matching MiniZero's getMCTSPolicy.
/// MiniZero formula: v_mix = (v̂_π_norm + (N / π_sum) * Σ_{a:N(a)>0} π(a)*q̂_norm(a)) / (1 + N)
/// where N = total simulation budget and π_sum = Σ_{a:N(a)>0} π(a).
fn compute_v_mix(v_pi: f32, priors: &[f32], tree: &Tree, k: usize, n_simulations: u32) -> f32 {
	let (q_min, q_max) = tree.q_bounds();
	let v_pi_norm = normalize_q(v_pi, q_min, q_max);
	let mut visited_prior_sum = 0.0f32;
	let mut weighted_q_sum = 0.0f32;
	for (i, &prior) in priors.iter().enumerate().take(k) {
		if tree.root_visited(i) {
			visited_prior_sum += prior;
			weighted_q_sum += prior * tree.root_q_normalized(i);
		}
	}
	if visited_prior_sum < 1e-8 {
		return v_pi_norm;
	}
	let n = n_simulations as f32;
	(v_pi_norm + (n / visited_prior_sum) * weighted_q_sum) / (1.0 + n)
}

/// Compute softmax and pair with moves.
fn softmax_to_moves(moves: &[Move], logits: &[f32]) -> Vec<(Move, f32)> {
	let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
	let exps: Vec<f32> = logits.iter().map(|&l| (l - max).exp()).collect();
	let sum: f32 = exps.iter().sum();
	moves.iter().copied().zip(exps.iter().map(|&e| e / sum)).collect()
}

impl<E, const N: usize> Bot<N> for GumbelMcts<E>
where
	E: Evaluator<N> + Send + Sync,
	[(); N * N]:,
	[(); N + 1]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let mut rng = rand::make_rng::<rand::rngs::SmallRng>();
		gumbel_search(game, &self.evaluator, &self.config, &mut rng).mv
	}
}

impl<E, const N: usize> SearchBot<E, N> for GumbelMcts<E>
where
	E: Evaluator<N> + Send + Sync,
	[(); N * N]:,
	[(); N + 1]:,
{
	fn with_sims(evaluator: E, sims: u32) -> Self {
		Self::new(
			evaluator,
			GumbelConfig {
				n_simulations: sims,
				m_actions: sims.min(16),
				..GumbelConfig::default()
			},
		)
	}
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng};
	use robot_master_arena::algos::rollout::Rollout;
	use robot_master_core::game::{GameConfig, GameState, Player, PlayerSigned};

	use super::*;
	use crate::mcts::RolloutEval;

	#[test]
	fn gumbel_returns_legal_move() {
		let mut rng = SmallRng::seed_from_u64(42);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
		let evaluator = RolloutEval::new(Rollout {});
		let config = GumbelConfig::default();

		let result = gumbel_search(&state, &evaluator, &config, &mut rng);

		assert!(state.valid_moves().any(|m| m == result.mv), "Gumbel returned illegal move");
	}

	#[test]
	fn gumbel_policy_target_sums_to_one() {
		let mut rng = SmallRng::seed_from_u64(7);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
		let evaluator = RolloutEval::new(Rollout {});
		let config = GumbelConfig {
			n_simulations: 8,
			m_actions: 8,
			..Default::default()
		};

		let result = gumbel_search(&state, &evaluator, &config, &mut rng);

		let sum: f32 = result.policy_target.iter().map(|(_, p)| p).sum();
		assert!((sum - 1.0).abs() < 1e-5, "policy target not normalized: {sum}");
	}
}
