use board_game::board::Board as _;
use robot_master_core::game::{GameState, Player};

use crate::{
	encoding::{action_index, action_size, encode_planes, encode_sample},
	mcts::{Evaluator, MctsConfig, search_visit_counts},
};

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
pub fn play_game<const N: usize, E>(state: &GameState<N>, evaluator: &E, config: &MctsConfig) -> Vec<Sample>
where
	E: Evaluator<N>,
	[(); N * N]:, {
	let mut game = state.clone();
	let mut pending: Vec<PendingSample> = Vec::with_capacity(GameState::<N>::total_moves());

	while game.outcome().is_none() {
		let planes = encode_planes(&game);
		let visit_counts = search_with_visits(&game, evaluator, config);
		let mover = game.turn;

		// normalize visit counts into a probability distribution
		let total: u32 = visit_counts.iter().sum();
		let policy: Vec<f32> = visit_counts.iter().map(|&v| v as f32 / total as f32).collect();

		// pick the most-visited move and advance game state
		let best_move = {
			let n = N;
			visit_counts
				.iter()
				.enumerate()
				.max_by_key(|&(_, &v)| v)
				.map(|(idx, _)| {
					let card = (idx / (n * n)) as u8;
					let flat = idx % (n * n);
					let row = flat / n;
					let col = flat % n;
					robot_master_core::game::Move {
						pos: robot_master_core::board::Pos { row: row as u8, col: col as u8 },
						card: robot_master_core::cards::CardValue(card),
					}
				})
				.expect("non-empty visit counts")
		};

		pending.push(PendingSample {
			state_planes: planes,
			policy,
			mover,
		});
		game.play(best_move).expect("MCTS selected illegal move");
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

/// Run MCTS from `state` and return a full visit-count vector over the action space.
///
/// Index: `card * N*N + row * N + col` — matches `action_index` in encoding.rs.
fn search_with_visits<const N: usize, E>(state: &GameState<N>, evaluator: &E, config: &MctsConfig) -> Vec<u32>
where
	E: Evaluator<N>,
	[(); N * N]:, {
	let counts = search_visit_counts(state, evaluator, config);
	let mut out = vec![0u32; action_size(N)];
	for (mv, count) in counts {
		let idx = action_index(mv.card.0, mv.pos.row as usize, mv.pos.col as usize, N);
		out[idx] = count;
	}
	out
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

	#[test]
	fn play_game_produces_correct_sample_count() {
		let mut rng = SmallRng::seed_from_u64(7);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng);
		let evaluator = RolloutEval::new(Rollout {});
		let config = MctsConfig { simulations: 10, c_puct: 1.41 };

		let samples = play_game(&state, &evaluator, &config);

		// 5x5 board: 24 moves per game (N*N - 1 = 24)
		assert_eq!(samples.len(), GameState::<5>::total_moves());
	}

	#[test]
	fn play_game_sample_shapes() {
		let mut rng = SmallRng::seed_from_u64(99);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng);
		let evaluator = RolloutEval::new(Rollout {});
		let config = MctsConfig { simulations: 5, c_puct: 1.41 };

		let samples = play_game(&state, &evaluator, &config);
		for s in &samples {
			assert_eq!(s.state_planes.len(), IN_CHANNELS * 25);
			assert_eq!(s.policy.len(), action_size(5));
			assert!(s.value == 1.0 || s.value == -1.0 || s.value == 0.0);
			let policy_sum: f32 = s.policy.iter().sum();
			assert!((policy_sum - 1.0).abs() < 1e-5, "policy not normalized: {policy_sum}");
		}
	}
}
