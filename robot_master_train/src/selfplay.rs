use board_game::board::Board as _;
use rand::Rng;
use robot_master_core::game::{GameState, Player};

use crate::{
	encoding::{action_index, action_size, encode_planes, encode_sample},
	gumbel::{GumbelConfig, gumbel_search},
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

/// Play one game using Gumbel AlphaZero search and return all training samples.
///
/// Policy targets are the completed-Q improved policy π' from each Gumbel search.
/// Values are filled in retroactively from the game outcome.
pub fn play_game<const N: usize, E, R>(state: &GameState<N>, evaluator: &E, config: &GumbelConfig, rng: &mut R) -> Vec<Sample>
where
	E: crate::mcts::Evaluator<N>,
	R: Rng,
	[(); N * N]:, {
	let mut game = state.clone();
	let mut pending: Vec<PendingSample> = Vec::with_capacity(GameState::<N>::total_moves());

	while game.outcome().is_none() {
		let planes = encode_planes(&game);
		let mover = game.turn;
		let result = gumbel_search(&game, evaluator, config, rng);

		// Map the per-move policy target to the full action space
		let mut policy = vec![0.0f32; action_size(N)];
		for (mv, prob) in result.policy_target {
			let idx = action_index(mv.card.0, mv.pos.row as usize, mv.pos.col as usize, N);
			policy[idx] = prob;
		}

		pending.push(PendingSample {
			state_planes: planes,
			policy,
			mover,
		});
		game.play(result.mv).expect("Gumbel selected illegal move");
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

struct PendingSample {
	state_planes: Vec<f32>,
	/// Completed-Q improved policy over all 6*N² actions.
	policy: Vec<f32>,
	mover: Player,
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng};
	use robot_master_arena::algos::rollout::Rollout;
	use robot_master_core::game::{GameConfig, GameState};

	use super::*;
	use crate::{
		encoding::{IN_CHANNELS, action_size},
		gumbel::GumbelConfig,
		mcts::RolloutEval,
	};

	fn config(sims: u32) -> GumbelConfig {
		GumbelConfig {
			n_simulations: sims,
			m_actions: sims.min(16),
			..Default::default()
		}
	}

	#[test]
	fn play_game_produces_correct_sample_count() {
		let mut rng = SmallRng::seed_from_u64(7);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng);
		let evaluator = RolloutEval::new(Rollout {});

		let samples = play_game(&state, &evaluator, &config(8), &mut rng);

		assert_eq!(samples.len(), GameState::<5>::total_moves());
	}

	#[test]
	fn play_game_sample_shapes() {
		let mut rng = SmallRng::seed_from_u64(99);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng);
		let evaluator = RolloutEval::new(Rollout {});

		let samples = play_game(&state, &evaluator, &config(4), &mut rng);
		for s in &samples {
			assert_eq!(s.state_planes.len(), IN_CHANNELS * 25);
			assert_eq!(s.policy.len(), action_size(5));
			assert!(s.value == 1.0 || s.value == -1.0 || s.value == 0.0);
			let policy_sum: f32 = s.policy.iter().sum();
			assert!((policy_sum - 1.0).abs() < 1e-5, "policy not normalized: {policy_sum}");
		}
	}
}
