use rand::Rng;
use robot_master_core::game::{GameConfig, GameState};

use crate::{match_::MatchResult, player::Player};

/// Run a round-robin tournament: every player plays every other player `rounds` times,
/// alternating who goes first.
///
/// Uses `dyn Player` — this is the orchestration layer, not the hot path.
/// For self-play training (millions of games), use `Match` directly with concrete types.
pub fn round_robin<const N: usize>(players: &mut [Box<dyn Player<N>>], config: GameConfig, rounds: usize, rng: &mut impl Rng) -> Vec<MatchResult>
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

/// Run a single match between two `dyn Player`s.
fn run_dyn_match<const N: usize>(mut game: GameState<N>, p1: &mut dyn Player<N>, p2: &mut dyn Player<N>) -> MatchResult
where
	[(); N * N]:, {
	use robot_master_core::{game::PlayerId, scoring::victoire};

	let mut moves = Vec::new();
	while !game.is_terminal() {
		let m = match game.turn {
			PlayerId::Cols => p1.choose_move(&game),
			PlayerId::Rows => p2.choose_move(&game),
		};
		game = game.apply_move(m).expect("illegal move in tournament");
		moves.push(m.into());
	}

	let (s0, i0, s1, i1) = victoire(&game.board);
	MatchResult {
		p1_id: p1.id(),
		p2_id: p2.id(),
		p1_score: s0,
		p2_score: s1,
		p1_weak_line: i0,
		p2_weak_line: i1,
		moves,
	}
}
