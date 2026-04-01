/// Gumbel AlphaZero search.
///
/// Replaces the standard MCTS + Dirichlet noise + temperature sampling used in AlphaGo Zero
/// with Sequential Halving guided by Gumbel noise. Key properties:
///
/// - Guarantees policy improvement: E[q(A_{n+1})] >= E_{a~π}[q(a)]
/// - Works reliably with n=2..32 simulations (vs 400+ for standard MCTS)
/// - No Dirichlet noise, no temperature schedule — exploration comes from Gumbel sampling
/// - Policy target = softmax(logits + σ(completedQ)), not visit counts
///
/// Reference: "Policy Improvement by Planning with Gumbel", Danihelka et al., ICLR 2022.
/// See docs/references/gumbel_alphazero.md for full details.
use rand::Rng;
use rand_distr::{Distribution as _, Gumbel};
use robot_master_arena::player::Bot;
use robot_master_core::game::{GameState, Move};

use crate::mcts::{Evaluator, SelectResult, Tree, expand_and_backprop, select, simulate};

pub struct GumbelConfig {
	/// Total simulation budget per move.
	pub n_simulations: u32,
	/// Number of root actions sampled without replacement (m ≤ n_simulations).
	/// Default: min(n_simulations, 16).
	pub m_actions: u32,
	/// c_visit in σ(q) = (c_visit + max_N) * c_scale * q. Paper default: 50.
	pub c_visit: f32,
	/// c_scale in σ. Paper default: 1.0 (use 0.1 if Q-values are unnormalized).
	pub c_scale: f32,
	/// c_puct for non-root PUCT selection (unchanged from standard MCTS).
	pub c_puct: f32,
}

impl Default for GumbelConfig {
	fn default() -> Self {
		let n = 16;
		Self {
			n_simulations: n,
			m_actions: n.min(16),
			c_visit: 50.0,
			c_scale: 1.0,
			c_puct: 1.41,
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
}

/// Run Gumbel AlphaZero search from `state`.
///
/// Returns the move to play and the completed-Q policy target for training.
pub fn gumbel_search<const N: usize, E, R>(state: &GameState<N>, evaluator: &E, config: &GumbelConfig, rng: &mut R) -> GumbelResult
where
	E: Evaluator<N>,
	R: Rng,
	[(); N * N]:, {
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

	// Step 1: sample k Gumbel(0,1) variables once — reused throughout
	let gumbel_dist = Gumbel::new(0.0f32, 1.0).expect("valid Gumbel params");
	let g: Vec<f32> = (0..k).map(|_| gumbel_dist.sample(rng)).collect();

	// Step 2: select top-m actions without replacement by g + logits
	let m = (config.m_actions as usize).min(k).min(config.n_simulations as usize).max(1);
	let gumbel_scores: Vec<f32> = (0..k).map(|i| g[i] + logits[i]).collect();
	let top_m: Vec<usize> = argtop_m(&gumbel_scores, m);

	// Step 3: Sequential Halving — allocate n simulations over top_m
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

		run_phase_batched(&mut tree, root_idx, state, evaluator, config.c_puct, &survivors, sims_per_action, n, &mut sims_used);

		if survivors.len() <= 1 {
			break;
		}

		// Rank survivors by g(a) + logits(a) + σ(q̂(a)), drop bottom half
		let max_visits = tree.max_root_visits();
		survivors.sort_unstable_by(|&a, &b| {
			let sa = gumbel_scores[a] + sigma(tree.root_q(a), max_visits, config);
			let sb = gumbel_scores[b] + sigma(tree.root_q(b), max_visits, config);
			sb.partial_cmp(&sa).expect("NaN in Gumbel score")
		});
		survivors.truncate((survivors.len() + 1) / 2);
	}

	// Spend any remaining budget on the last survivor(s)
	while sims_used < n {
		simulate(&mut tree, root_idx, Some(survivors[0]), state, evaluator, config.c_puct);
		sims_used += 1;
	}

	// Step 4: select A_{n+1} — argmax of g + logits + σ(q̂) among survivors
	let max_visits = tree.max_root_visits();
	let best_idx = *survivors
		.iter()
		.max_by(|&&a, &&b| {
			let sa = gumbel_scores[a] + sigma(tree.root_q(a), max_visits, config);
			let sb = gumbel_scores[b] + sigma(tree.root_q(b), max_visits, config);
			sa.partial_cmp(&sb).expect("NaN in Gumbel score")
		})
		.expect("survivors non-empty");

	// Step 5: compute policy target π' = softmax(logits + σ(completedQ))
	let v_mix = compute_v_mix(root_value, &priors, &tree, k);
	let q_norm_range = tree.q_range(root_value);
	let completed_q: Vec<f32> = (0..k)
		.map(|i| {
			let raw_q = if tree.root_visited(i) { tree.root_q_raw(i) } else { v_mix };
			normalize_q(raw_q, q_norm_range)
		})
		.collect();

	let max_visits_f = max_visits as f32;
	let improved_logits: Vec<f32> = (0..k).map(|i| logits[i] + (config.c_visit + max_visits_f) * config.c_scale * completed_q[i]).collect();
	let policy_target = softmax_to_moves(&moves, &improved_logits);

	GumbelResult { mv: moves[best_idx], policy_target }
}

/// Run one phase of Sequential Halving using batched leaf evaluation.
///
/// Within a phase, all simulations that need NN evaluation are batched together.
/// The trick: each forced root action leads to a distinct child of the root that is
/// unexpanded at first visit. Once expanded, deeper sims may hit already-expanded
/// nodes and fall through to terminal/re-visit — those are handled individually.
///
/// Because each `action_idx` in `survivors` is a distinct root edge, multiple sims
/// for the same action within a phase can share the same evaluation result for the
/// first expansion (the direct child). Subsequent sims on an already-expanded action
/// descend deeper into its subtree — those may or may not hit new leaves.
///
/// Strategy: collect all `(action_idx, sim)` selections that return NeedsEval into a
/// batch, evaluate once, then expand+backprop. Sims that hit terminals are handled
/// inline without NN calls.
fn run_phase_batched<const N: usize, E>(
	tree: &mut Tree,
	root_idx: u32,
	state: &GameState<N>,
	evaluator: &E,
	c_puct: f32,
	survivors: &[usize],
	sims_per_action: usize,
	budget: usize,
	sims_used: &mut usize,
) where
	E: Evaluator<N>,
	[(); N * N]:, {
	struct Pending<const M: usize>
	where
		[(); M * M]:, {
		path: Vec<u32>,
		parent: u32,
		edge_idx: usize,
		leaf_state: GameState<M>,
	}

	// Collect all sims that need NN evaluation. Sims that hit already-expanded or
	// terminal nodes are handled eagerly via `simulate` (no NN needed).
	//
	// We run selection against the *current* tree state, which means each sim sees
	// the expansions from previous iterations/phases — this is correct. Within a
	// single phase batch, we collect leaves without yet expanding them, so subsequent
	// selections in the same batch may select the same unexpanded edge. We deduplicate
	// by (parent, edge_idx): if an edge is already in the pending list we fall back to
	// simulate() for that sim (it will re-select from the same node using PUCT).
	let mut pending: Vec<Pending<N>> = Vec::new();
	let mut pending_edges: std::collections::HashSet<(u32, usize)> = std::collections::HashSet::new();

	for &action_idx in survivors {
		for _ in 0..sims_per_action {
			if *sims_used >= budget {
				break;
			}
			*sims_used += 1;

			match select(tree, root_idx, Some(action_idx), state, c_puct) {
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
						simulate(tree, root_idx, Some(action_idx), state, evaluator, c_puct);
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

/// Gumbel-based bot: wraps `gumbel_search` and implements `Bot<N>`.
pub struct GumbelBot<E> {
	evaluator: E,
	config: GumbelConfig,
}
impl<E> GumbelBot<E> {
	pub fn new(evaluator: E, config: GumbelConfig) -> Self {
		Self { evaluator, config }
	}
}

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

/// Normalize a raw Q value to [0,1] using the tree's min/max range.
fn normalize_q(raw_q: f32, (q_min, q_max): (f32, f32)) -> f32 {
	if (q_max - q_min).abs() < 1e-8 {
		0.5
	} else {
		((raw_q - q_min) / (q_max - q_min)).clamp(0.0, 1.0)
	}
}

/// v_mix: interpolation between v̂_π and prior-weighted average of observed Q-values.
/// Appendix D, Eq. 33 from the paper.
fn compute_v_mix(v_pi: f32, priors: &[f32], tree: &Tree, k: usize) -> f32 {
	let mut visited_prior_sum = 0.0f32;
	let mut weighted_q_sum = 0.0f32;
	for i in 0..k {
		if tree.root_visited(i) {
			visited_prior_sum += priors[i];
			weighted_q_sum += priors[i] * tree.root_q_raw(i);
		}
	}
	if visited_prior_sum < 1e-8 {
		return v_pi;
	}
	(v_pi + weighted_q_sum / visited_prior_sum) / 2.0
}

/// Compute softmax and pair with moves.
fn softmax_to_moves(moves: &[Move], logits: &[f32]) -> Vec<(Move, f32)> {
	let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
	let exps: Vec<f32> = logits.iter().map(|&l| (l - max).exp()).collect();
	let sum: f32 = exps.iter().sum();
	moves.iter().copied().zip(exps.iter().map(|&e| e / sum)).collect()
}

impl<E, const N: usize> Bot<N> for GumbelBot<E>
where
	E: Evaluator<N> + Send + Sync,
	[(); N * N]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let mut rng = rand::make_rng::<rand::rngs::SmallRng>();
		gumbel_search(game, &self.evaluator, &self.config, &mut rng).mv
	}
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng};
	use robot_master_arena::algos::rollout::Rollout;
	use robot_master_core::game::{GameConfig, GameState};

	use super::*;
	use crate::mcts::RolloutEval;

	#[test]
	fn gumbel_returns_legal_move() {
		let mut rng = SmallRng::seed_from_u64(42);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng);
		let evaluator = RolloutEval::new(Rollout {});
		let config = GumbelConfig::default();

		let result = gumbel_search(&state, &evaluator, &config, &mut rng);

		assert!(state.valid_moves().any(|m| m == result.mv), "Gumbel returned illegal move");
	}

	#[test]
	fn gumbel_policy_target_sums_to_one() {
		let mut rng = SmallRng::seed_from_u64(7);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng);
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
