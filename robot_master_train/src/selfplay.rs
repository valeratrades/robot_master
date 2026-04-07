use board_game::board::{Board as _, Outcome};
use rand::Rng;
use robot_master_core::game::{GameConfig, GameState, Player, PlayerSigned};

use crate::{
	encoding::{action_index, action_size, encode_planes, encode_sample},
	gumbel::{GumbelConfig, GumbelSearch, gumbel_search, gumbel_setup},
	mcts::Evaluator,
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
/// Value targets are the game outcome from the mover's perspective (±1/0).
pub fn play_game<const N: usize, E, R>(state: &GameState<N>, evaluator: &E, config: &GumbelConfig, rng: &mut R) -> Vec<Sample>
where
	E: crate::mcts::Evaluator<N>,
	R: Rng,
	[(); N * N]:,
	[(); N + 1]:, {
	let mut game = state.clone();
	let mut pending: Vec<PendingSample> = Vec::with_capacity(GameState::<N>::total_moves());

	while game.outcome().is_none() {
		let planes = encode_planes(&game);
		let mover = game.turn;
		let result = gumbel_search(&game, evaluator, config, rng);

		// Map the per-move policy target to the full action space
		let mut policy = vec![0.0f32; action_size(N)];
		for (mv, prob) in result.policy_target {
			let idx = action_index(mv.card.0, mv.pos.row as usize, mv.pos.col as usize, N, mover == Player::B);
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

/// Play `total_games` games using a pool of `batch_size` concurrent in-flight
/// games. All NN evaluations are collected across the pool and dispatched in a
/// single `evaluate_batch` call per step, keeping GPU utilization high.
///
/// Yields completed game sample batches as each game finishes. Games are
/// independent — `rng` is used to seed per-game Gumbel noise, but each game's
/// tree state is isolated.
///
/// # Invariants
/// - Each game's Gumbel search is still sequential within itself (phases are
///   driven one at a time; tree state is consistent before each selection run).
/// - `evaluate_batch` is called at most once per loop iteration.
/// - The total number of NN calls equals (root evals + leaf evals) × total_games,
///   same as the sequential path — just batched differently.
pub fn play_games_batched<const N: usize, E, R>(total_games: usize, evaluator: &E, config: &GumbelConfig, rng: &mut R, batch_size: usize, game_config: GameConfig) -> Vec<Vec<Sample>>
where
	E: Evaluator<N>,
	R: Rng,
	[(); N * N]:,
	[(); N + 1]:, {
	// Each slot is either active (Some) or available for a new game (None).
	let mut slots: Vec<Option<GameInFlight<N>>> = (0..batch_size).map(|_| None).collect();
	let mut games_started = 0usize;
	let mut completed: Vec<Vec<Sample>> = Vec::with_capacity(total_games);

	// Fill initial slots
	for slot in &mut slots {
		if games_started >= total_games {
			break;
		}
		*slot = Some(GameInFlight::new(rng, game_config));
		games_started += 1;
	}

	//LOOP: exits via `break` when all_idle (all slots empty = all games finished)
	loop {
		// --- Phase 1: batch root evaluations for games that just started ---
		// Newly created GameInFlight starts with state=NeedsRootEval.
		let root_indices: Vec<usize> = slots
			.iter()
			.enumerate()
			.filter_map(|(i, s)| if matches!(s, Some(g) if g.needs_root_eval()) { Some(i) } else { None })
			.collect();

		if !root_indices.is_empty() {
			let root_states: Vec<GameState<N>> = root_indices.iter().map(|&i| slots[i].as_ref().unwrap().current_state().clone()).collect();
			let root_evals = evaluator.evaluate_batch(&root_states);
			for (&slot_idx, eval) in root_indices.iter().zip(root_evals) {
				slots[slot_idx].as_mut().unwrap().start_search(eval, config, rng);
			}
		}

		// --- Phase 2: collect leaf selections across all active searches ---
		// Each active game runs its current search phase's selection loop,
		// accumulating leaves that need NN eval. Terminals are handled inline.
		let mut leaf_slot_indices: Vec<usize> = Vec::new();
		let mut leaf_states: Vec<GameState<N>> = Vec::new();
		let mut leaf_counts: Vec<usize> = Vec::new(); // how many leaves each slot contributed

		for (i, slot) in slots.iter_mut().enumerate() {
			let Some(game) = slot else { continue };
			if game.needs_root_eval() || game.search_done() {
				continue; // not yet started search or already finished this move
			}
			let before = leaf_states.len();
			let pending = game.search.as_mut().unwrap().collect_pending_selections();
			for leaf in pending {
				leaf_states.push(leaf.leaf_state.clone());
			}
			let count = leaf_states.len() - before;
			if count > 0 {
				leaf_slot_indices.push(i);
				leaf_counts.push(count);
			}
		}

		// Batch eval all leaves
		if !leaf_states.is_empty() {
			let evals = evaluator.evaluate_batch(&leaf_states);
			let mut eval_cursor = 0;
			for (&slot_idx, &count) in leaf_slot_indices.iter().zip(leaf_counts.iter()) {
				let batch = evals[eval_cursor..eval_cursor + count].to_vec();
				eval_cursor += count;
				slots[slot_idx].as_mut().unwrap().search.as_mut().unwrap().apply_evals(batch);
			}
		}

		// --- Phase 3: advance games whose search step just completed ---
		for slot in slots.iter_mut() {
			let Some(game) = slot else { continue };
			game.try_advance_move();
		}

		// --- Phase 4: retire finished games, start new ones ---
		let mut all_idle = true;
		for slot in slots.iter_mut() {
			match slot {
				None => {}
				Some(game) if game.is_terminal() => {
					all_idle = false;
					let samples = slot.take().unwrap().finish();
					completed.push(samples);
					if games_started < total_games {
						*slot = Some(GameInFlight::new(rng, game_config));
						games_started += 1;
					}
				}
				Some(_) => {
					all_idle = false;
				}
			}
		}

		if all_idle {
			break;
		}
	}

	completed
}
struct PendingSample {
	state_planes: Vec<f32>,
	/// Completed-Q improved policy over all 6*N² actions.
	policy: Vec<f32>,
	mover: Player,
}

// ---------------------------------------------------------------------------
// Vectorized (GPU-batched) self-play
// ---------------------------------------------------------------------------

/// State machine for one in-flight game within the vectorized pool.
enum GamePhase<const N: usize>
where
	[(); N * N]:,
	[(); N + 1]:, {
	/// Waiting for the root NN evaluation of `state`.
	NeedsRootEval { state: GameState<N> },
	/// Root evaluated, Gumbel search in progress.
	Searching,
	/// Game finished (terminal state reached).
	Terminal,
}

struct GameInFlight<const N: usize>
where
	[(); N * N]:,
	[(); N + 1]:, {
	phase: GamePhase<N>,
	/// Current game state (updated after each move).
	game: GameState<N>,
	/// Samples accumulated so far (pending value backfill).
	pending_samples: Vec<PendingSample>,
	/// Active Gumbel search for the current move, if any.
	search: Option<GumbelSearch<N>>,
	/// Whether the game has reached a terminal state.
	terminal: bool,
}

impl<const N: usize> GameInFlight<N>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn new(rng: &mut impl Rng, game_config: GameConfig) -> Self {
		let game = GameState::<N>::new(game_config, rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
		let state = game.clone();
		Self {
			phase: GamePhase::NeedsRootEval { state },
			game,
			pending_samples: Vec::new(),
			search: None,
			terminal: false,
		}
	}

	fn needs_root_eval(&self) -> bool {
		matches!(self.phase, GamePhase::NeedsRootEval { .. })
	}

	fn search_done(&self) -> bool {
		self.search.as_ref().map_or(true, |s| s.is_done())
	}

	fn is_terminal(&self) -> bool {
		self.terminal
	}

	fn current_state(&self) -> &GameState<N> {
		match &self.phase {
			GamePhase::NeedsRootEval { state } => state,
			_ => &self.game,
		}
	}

	/// Called after the root eval arrives. Sets up the GumbelSearch for this move.
	fn start_search(&mut self, root_eval: crate::mcts::Evaluation, config: &GumbelConfig, rng: &mut impl Rng) {
		let state = self.game.clone();
		// Record the pre-move state planes and mover for the sample.
		// value is filled in try_advance_move once root_mean is available.
		let planes = encode_planes(&state);
		let mover = state.turn;
		self.pending_samples.push(PendingSample {
			state_planes: planes,
			policy: vec![],
			mover,
		});

		let (gumbel_scores, moves, priors) = gumbel_setup::<N, _>(&root_eval, rng);
		self.search = Some(GumbelSearch::new(&state, root_eval, config.clone(), gumbel_scores, moves, priors));
		self.phase = GamePhase::Searching;
	}

	/// If the current search just finished, record the move and set up for the next one.
	fn try_advance_move(&mut self) {
		let search_done = self.search.as_ref().map_or(false, |s| s.is_done());
		if !search_done {
			return;
		}

		let result = self.search.take().unwrap().finish();

		// Fill in the policy target for the sample we pushed in start_search.
		let last = self.pending_samples.last_mut().expect("sample was pushed in start_search");
		let mut policy = vec![0.0f32; action_size(N)];
		for (mv, prob) in result.policy_target {
			let idx = action_index(mv.card.0, mv.pos.row as usize, mv.pos.col as usize, N, last.mover == Player::B);
			policy[idx] = prob;
		}
		last.policy = policy;

		self.game.play(result.mv).expect("Gumbel selected illegal move");

		if self.game.outcome().is_some() {
			self.terminal = true;
			self.phase = GamePhase::Terminal;
		} else {
			// Queue the next move's root eval
			let next_state = self.game.clone();
			self.phase = GamePhase::NeedsRootEval { state: next_state };
		}
	}

	/// Consume this game and produce its training samples.
	fn finish(self) -> Vec<Sample> {
		let outcome = self.game.outcome().expect("finish called on non-terminal game");
		self.pending_samples
			.into_iter()
			.map(|s| {
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
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng};
	use robot_master_arena::algos::rollout::Rollout;
	use robot_master_core::game::{GameConfig, GameState, Player, PlayerSigned};

	use super::*;
	use crate::{
		encoding::{action_size, in_channels},
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
		let state = GameState::<5>::new(GameConfig::default(), &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
		let evaluator = RolloutEval::new(Rollout {});

		let samples = play_game(&state, &evaluator, &config(8), &mut rng);

		assert_eq!(samples.len(), GameState::<5>::total_moves());
	}

	#[test]
	fn play_game_sample_shapes() {
		let mut rng = SmallRng::seed_from_u64(99);
		let state = GameState::<5>::new(GameConfig::default(), &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
		let evaluator = RolloutEval::new(Rollout {});

		let samples = play_game(&state, &evaluator, &config(4), &mut rng);
		for s in &samples {
			assert_eq!(s.state_planes.len(), in_channels(5) * 25);
			assert_eq!(s.policy.len(), action_size(5));
			assert!(s.value == 1.0 || s.value == -1.0 || s.value == 0.0);
			let policy_sum: f32 = s.policy.iter().sum();
			assert!((policy_sum - 1.0).abs() < 1e-5, "policy not normalized: {policy_sum}");
		}
	}
}
