use board_game::board::Board as _;
use rand::RngExt as _;
use rand_distr::{Distribution as _, multi::Dirichlet};
use robot_master_core::game::{GameState, Player};

use crate::{
	encoding::{action_index, action_size, encode_planes, encode_sample},
	mcts::{Evaluation, Evaluator, MctsConfig, search_visit_counts},
};

/// Self-play configuration: MCTS settings + exploration parameters.
pub struct SelfplayConfig {
	pub mcts: MctsConfig,
	/// Number of moves at the start of the game where τ=1 (sample ∝ visit counts).
	/// After this, τ→0 (greedy argmax). AlphaGo Zero uses 30.
	pub temp_moves: usize,
	/// Dirichlet concentration α for root noise. Scaled inverse to branching factor:
	/// chess ~0.3, shogi ~0.15, Go ~0.03. Robot Master 5x5 (~25 moves) ≈ 0.3.
	pub dirichlet_alpha: f32,
	/// Weight of Dirichlet noise mixed into root priors: P = (1-ε)·p + ε·η.
	/// AlphaZero/AlphaGo Zero use ε = 0.25.
	pub dirichlet_epsilon: f32,
}

impl Default for SelfplayConfig {
	fn default() -> Self {
		Self {
			mcts: MctsConfig::default(),
			temp_moves: 30,
			dirichlet_alpha: 0.3,
			dirichlet_epsilon: 0.25,
		}
	}
}

/// A completed training sample ready for serialization.
pub struct Sample {
	pub state_planes: Vec<f32>,
	pub policy: Vec<f32>,
	/// +1.0 if the player who was to move at this step won, -1.0 if they lost.
	pub value: f32,
}
impl Sample {
	pub fn to_bytes(&self) -> Vec<u8> {
		encode_sample(&self.state_planes, &self.policy, self.value)
	}
}

/// Play one game using the given evaluator and return all training samples.
///
/// Both players use the same evaluator and config (self-play).
/// Policy targets are MCTS visit count distributions over the full action space.
/// Values are filled in retroactively from the game outcome.
pub fn play_game<const N: usize, E, R>(state: &GameState<N>, evaluator: &E, config: &SelfplayConfig, rng: &mut R) -> Vec<Sample>
where
	E: Evaluator<N>,
	R: rand::Rng,
	[(); N * N]:, {
	let mut game = state.clone();
	let mut pending: Vec<PendingSample> = Vec::with_capacity(GameState::<N>::total_moves());
	let mut move_num: usize = 0;

	while game.outcome().is_none() {
		let planes = encode_planes(&game);
		let root_eval = add_dirichlet_noise(evaluator.evaluate(&game), config, rng);
		let visit_counts = search_with_visits_from_eval(&game, evaluator, &config.mcts, root_eval);
		let mover = game.turn;

		// normalize visit counts into a probability distribution (policy target)
		let total: u32 = visit_counts.iter().sum();
		let policy: Vec<f32> = visit_counts.iter().map(|&v| v as f32 / total as f32).collect();

		let played_move = if move_num < config.temp_moves {
			// τ=1: sample ∝ visit counts
			sample_from_visit_counts(&visit_counts, rng)
		} else {
			// τ→0: greedy argmax
			argmax_move(&visit_counts)
		};

		pending.push(PendingSample {
			state_planes: planes,
			policy,
			mover,
		});
		game.play(played_move).expect("MCTS selected illegal move");
		move_num += 1;
	}

	let outcome = game.outcome().expect("game must be finished");
	pending
		.into_iter()
		.map(|s| {
			use board_game::board::Outcome;
			let value = match outcome {
				Outcome::WonBy(winner) if winner == s.mover => 1.0,
				Outcome::WonBy(_) => -1.0,
				Outcome::Draw => 0.0,
			};
			Sample {
				state_planes: s.state_planes,
				policy: s.policy,
				value,
			}
		})
		.collect()
}
/// One training sample. `value` is filled in retroactively after the game ends.
struct PendingSample {
	state_planes: Vec<f32>,
	/// Visit count distribution over all 6*N^2 actions, normalized.
	policy: Vec<f32>,
	/// Which player was to move when this sample was recorded.
	mover: Player,
}

/// Run MCTS from `state` with a pre-computed (possibly noise-perturbed) root evaluation,
/// and return a full visit-count vector over the action space.
///
/// Index: `card * N*N + row * N + col` — matches `action_index` in encoding.rs.
fn search_with_visits_from_eval<const N: usize, E>(state: &GameState<N>, evaluator: &E, config: &MctsConfig, root_eval: Evaluation) -> Vec<u32>
where
	E: Evaluator<N>,
	[(); N * N]:, {
	let counts = search_visit_counts(state, evaluator, config, root_eval);
	let mut out = vec![0u32; action_size(N)];
	for (mv, count) in counts {
		let idx = action_index(mv.card.0, mv.pos.row as usize, mv.pos.col as usize, N);
		out[idx] = count;
	}
	out
}

/// Mix Dirichlet noise into the root evaluation's policy priors.
/// P(s, a) = (1 - ε) * p_a  +  ε * η_a, where η ~ Dir(α).
fn add_dirichlet_noise<R: rand::Rng>(mut eval: Evaluation, config: &SelfplayConfig, rng: &mut R) -> Evaluation {
	let k = eval.policy.len();
	if k < 2 {
		return eval;
	}
	let alpha = vec![config.dirichlet_alpha; k];
	let dir = Dirichlet::new(&alpha).expect("valid Dirichlet params");
	let noise: Vec<f32> = dir.sample(rng);
	let eps = config.dirichlet_epsilon;
	for (pair, eta) in eval.policy.iter_mut().zip(noise) {
		pair.1 = (1.0 - eps) * pair.1 + eps * eta;
	}
	eval
}

/// Sample a move index proportional to visit counts (τ=1).
fn sample_from_visit_counts<R: rand::Rng>(visit_counts: &[u32], rng: &mut R) -> robot_master_core::game::Move {
	let total: u32 = visit_counts.iter().sum();
	assert!(total > 0, "all visit counts are zero");
	let threshold = rng.random_range(0..total);
	let mut cumsum: u32 = 0;
	for (idx, &v) in visit_counts.iter().enumerate() {
		cumsum += v;
		if cumsum > threshold {
			return idx_to_move(idx, visit_counts.len());
		}
	}
	// Fallback: rounding shouldn't reach here, but if it does, pick last non-zero.
	let idx = visit_counts.iter().rposition(|&v| v > 0).expect("non-zero visit counts");
	idx_to_move(idx, visit_counts.len())
}

/// Pick the move with the most visits (τ→0).
fn argmax_move(visit_counts: &[u32]) -> robot_master_core::game::Move {
	let idx = visit_counts.iter().enumerate().max_by_key(|&(_, v)| v).map(|(i, _)| i).expect("non-empty visit counts");
	idx_to_move(idx, visit_counts.len())
}

/// Convert a flat action index back into a Move. Index layout: card * N² + row * N + col.
fn idx_to_move(idx: usize, action_space: usize) -> robot_master_core::game::Move {
	// action_space = 6 * N², so N² = action_space / 6
	let n_sq = action_space / 6;
	let n = (n_sq as f64).sqrt().round() as usize;
	let card = (idx / n_sq) as u8;
	let flat = idx % n_sq;
	robot_master_core::game::Move {
		pos: robot_master_core::board::Pos {
			row: (flat / n) as u8,
			col: (flat % n) as u8,
		},
		card: robot_master_core::cards::CardValue(card),
	}
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng};
	use robot_master_arena::algos::rollout::Rollout;
	use robot_master_core::game::{GameConfig, GameState};

	use super::*;
	use crate::{
		encoding::{IN_CHANNELS, action_size},
		mcts::{MctsConfig, RolloutEval},
	};

	fn config(sims: u32) -> SelfplayConfig {
		SelfplayConfig {
			mcts: MctsConfig { simulations: sims, c_puct: 1.41 },
			..Default::default()
		}
	}

	#[test]
	fn play_game_produces_correct_sample_count() {
		let mut rng = SmallRng::seed_from_u64(7);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng);
		let evaluator = RolloutEval::new(Rollout {});

		let samples = play_game(&state, &evaluator, &config(10), &mut rng);

		// 5x5 board: 24 moves per game (N*N - 1 = 24)
		assert_eq!(samples.len(), GameState::<5>::total_moves());
	}

	#[test]
	fn play_game_sample_shapes() {
		let mut rng = SmallRng::seed_from_u64(99);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng);
		let evaluator = RolloutEval::new(Rollout {});

		let samples = play_game(&state, &evaluator, &config(5), &mut rng);
		for s in &samples {
			assert_eq!(s.state_planes.len(), IN_CHANNELS * 25);
			assert_eq!(s.policy.len(), action_size(5));
			assert!(s.value == 1.0 || s.value == -1.0 || s.value == 0.0);
			let policy_sum: f32 = s.policy.iter().sum();
			assert!((policy_sum - 1.0).abs() < 1e-5, "policy not normalized: {policy_sum}");
		}
	}
}
