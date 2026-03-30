use rand::Rng;
use robot_master_core::game::{GameConfig, GameState};

use crate::{
	match_::{Match, MatchResult},
	player::Bot,
};

/// Run a round-robin tournament: every player plays every other player `rounds` times,
/// alternating who goes first.
///
/// Uses `dyn Player` — this is the orchestration layer, not the hot path.
/// For self-play training (millions of games), use `Match` directly with concrete types.
pub fn round_robin<const N: usize>(players: &mut [Box<dyn Bot<N>>], config: GameConfig, rounds: usize, rng: &mut impl Rng) -> Vec<MatchResult>
where
	[(); N * N]:, {
	let n = players.len();
	let mut results = Vec::new();

	for round in 0..rounds {
		for i in 0..n {
			for j in (i + 1)..n {
				// Alternate who plays first each round
				let (p1_idx, p2_idx) = if round % 2 == 0 { (i, j) } else { (j, i) };

				let game = GameState::<N>::new(config, rng);

				// We need to borrow two players mutably at once.
				// Safe because p1_idx != p2_idx.
				assert!(p1_idx != p2_idx);
				let (p1, p2) = if p1_idx < p2_idx {
					let (left, right) = players.split_at_mut(p2_idx);
					(&mut *left[p1_idx], &mut *right[0])
				} else {
					let (left, right) = players.split_at_mut(p1_idx);
					(&mut *right[0], &mut *left[p2_idx])
				};

				let result = run_dyn_match::<N>(game, p1, p2);
				results.push(result);
			}
		}
	}

	results
}

fn run_dyn_match<const N: usize>(game: GameState<N>, p1: &mut dyn Bot<N>, p2: &mut dyn Bot<N>) -> MatchResult
where
	[(); N * N]:, {
	Match::new(game, p1, p2).run()
}
