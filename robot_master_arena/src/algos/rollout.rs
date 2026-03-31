use board_game::board::Board as _;
use bon::Builder;
use robot_master_core::{
	board::{Board, EMPTY},
	game::{GameState, Move, Player, scores_rows},
	scoring::{line_counts, score_delta, score_line},
};
use ustr::{Ustr, ustr};
use v_utils::macros::CompactFormatNamed;

use crate::player::Bot;

/// Rollout-quality bot: `modal_greed` when we can improve our score, `averse` when we can't.
#[derive(Clone, CompactFormatNamed, Debug)]
pub struct Rollout {}

impl<const N: usize> Bot<N> for Rollout
where
	[(); N * N]:,
{
	fn id(&self) -> Ustr {
		ustr(&self.to_string())
	}

	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let analysis = LineAnalysis::new(game);

		if analysis.can_improve() {
			modal_greed(game, &analysis)
		} else {
			Averse::builder().fuel(500).build().choose(game)
		}
	}
}

struct LineAnalysis {
	/// (line_index, score, is_finished)
	lines: Vec<(usize, u16, bool)>,
	finished_min: Option<u16>,
}

impl LineAnalysis {
	fn new<const N: usize>(game: &GameState<N>) -> Self
	where
		[(); N * N]:, {
		let player = game.turn;
		let lines: Vec<_> = (0..N)
			.map(|i| {
				let line = game.board.line(player, i);
				let counts = line_counts(&line);
				let score = score_line(&counts);
				let is_finished = line.iter().all(|&c| c != EMPTY);
				(i, score, is_finished)
			})
			.collect();

		let finished_min = lines.iter().filter(|l| l.2).map(|l| l.1).min();

		Self { lines, finished_min }
	}

	/// True if there exists an unfinished line scoring below the finished minimum,
	/// or if no lines are finished yet (everything is still improvable).
	fn can_improve(&self) -> bool {
		let Some(fmin) = self.finished_min else {
			return true; // no finished lines — mode 1
		};
		self.lines.iter().any(|&(_, score, finished)| !finished && score < fmin)
	}
}

/// Modes 1+2: greedy play focused on our own lines.
///
/// Mode 1 (no finished lines): maximize score_delta on weakest line.
/// Mode 2 (unfinished bottleneck): maximize score_delta on unfinished lines below finished_min.
fn modal_greed<const N: usize>(game: &GameState<N>, analysis: &LineAnalysis) -> Move
where
	[(); N * N]:, {
	let player = game.turn;
	let hand = &game.hands[player.index() as usize];
	let board = &game.board;

	// Which lines are we trying to improve?
	let target_lines: Vec<usize> = match analysis.finished_min {
		None => {
			// Mode 1: all lines are targets; focus on the weakest.
			let min_score = analysis.lines.iter().map(|l| l.1).min().expect("no lines");
			analysis.lines.iter().filter(|l| l.1 == min_score).map(|l| l.0).collect()
		}
		Some(fmin) => {
			// Mode 2: only unfinished lines below fmin.
			analysis.lines.iter().filter(|l| !l.2 && l.1 < fmin).map(|l| l.0).collect()
		}
	};

	let mut best: Option<(i16, Move)> = None;

	for pos in board.valid_placements() {
		let line_idx = if scores_rows(player) { pos.row as usize } else { pos.col as usize };
		if !target_lines.contains(&line_idx) {
			continue;
		}

		let counts = line_counts(&board.line(player, line_idx));
		for card in hand.iter_playable() {
			let delta = score_delta(&counts, card);
			if best.is_none_or(|b| delta > b.0) {
				best = Some((delta, Move { pos, card }));
			}
		}
	}

	// If no target line has a playable position, fall back to best delta on any line.
	best.map(|b| b.1).unwrap_or_else(|| greedy_any(game))
}

/// Fallback greedy: best score_delta on any of our lines.
fn greedy_any<const N: usize>(game: &GameState<N>) -> Move
where
	[(); N * N]:, {
	let player = game.turn;
	let hand = &game.hands[player.index() as usize];
	let board = &game.board;

	let mut best: Option<(i16, Move)> = None;

	for pos in board.valid_placements() {
		let line_idx = if scores_rows(player) { pos.row as usize } else { pos.col as usize };
		let counts = line_counts(&board.line(player, line_idx));
		for card in hand.iter_playable() {
			let delta = score_delta(&counts, card);
			if best.is_none_or(|b| delta > b.0) {
				best = Some((delta, Move { pos, card }));
			}
		}
	}

	best.expect("no valid moves").1
}

/// Bounded sadist: minimizes opponent's max potential, but uses `modal_greed` for opponent
/// modeling instead of exhaustive enumeration, and stops after `fuel` iterations.
#[derive(Builder)]
struct Averse {
	#[builder(default = 1000)]
	fuel: u32,
}

impl Averse {
	fn choose<const N: usize>(&self, game: &GameState<N>) -> Move
	where
		[(); N * N]:, {
		let opponent = game.turn.other();
		let mut best_move: Option<Move> = None;
		let mut best_opp_score: Option<u16> = None;
		let mut fuel_remaining = self.fuel;

		for m in game.valid_moves() {
			if fuel_remaining == 0 {
				break;
			}
			fuel_remaining -= 1;

			let next = game.clone_and_play(m).expect("valid_moves produced illegal move");
			let opp_score = project_opponent_score::<N>(&next, opponent);

			if best_opp_score.is_none_or(|bs| opp_score < bs) {
				best_opp_score = Some(opp_score);
				best_move = Some(m);
			}
		}

		best_move.expect("no valid moves")
	}
}

/// Project what score the opponent could reach if they play modal_greed from here.
/// Simulates opponent turns only (skips our turns with greedy_any) until the game ends,
/// then returns opponent's final score.
fn project_opponent_score<const N: usize>(game: &GameState<N>, opponent: Player) -> u16
where
	[(); N * N]:, {
	let mut sim = game.clone();

	while sim.outcome().is_none() {
		let analysis = LineAnalysis::new(&sim);
		let mv = if sim.turn == opponent {
			if analysis.can_improve() { modal_greed(&sim, &analysis) } else { greedy_any(&sim) }
		} else {
			greedy_any(&sim)
		};
		sim.play(mv).expect("projected illegal move");
	}

	opponent_min_score(&sim.board, opponent)
}

fn opponent_min_score<const N: usize>(board: &Board<N>, player: Player) -> u16
where
	[(); N * N]:, {
	(0..N).map(|i| score_line(&line_counts(&board.line(player, i)))).min().expect("no lines")
}

#[cfg(test)]
mod tests {
	use board_game::board::Board as _;
	use insta::assert_snapshot;
	use rand::{SeedableRng, rngs::SmallRng};
	use robot_master_core::{
		board::{Board, Pos},
		cards::{CardValue, Hand},
		game::{GameConfig, GameState},
	};

	use super::*;

	fn make_state(grid: [[Option<u8>; 5]; 5], hand: Hand, turn: Player) -> GameState<5> {
		let mut board = Board::<5>::default();
		for row in 0..5u8 {
			for col in 0..5u8 {
				if let Some(v) = grid[row as usize][col as usize] {
					board.set(Pos { row, col }, v);
				}
			}
		}
		GameState {
			board,
			hands: match turn {
				Player::A => [hand, Hand::default()],
				Player::B => [Hand::default(), hand],
			},
			turn,
			config: GameConfig::default(),
		}
	}

	fn hand(pairs: &[(u8, u8)]) -> Hand {
		let mut h = Hand::default();
		for &(v, n) in pairs {
			for _ in 0..n {
				h.put(CardValue(v));
			}
		}
		h
	}

	#[test]
	fn rollout_returns_legal_move() {
		let mut rng = SmallRng::seed_from_u64(42);
		let state: GameState<5> = GameState::new(GameConfig::default(), &mut rng);
		let mv = Rollout {}.choose_move(&state);
		assert!(state.valid_moves().any(|m| m == mv), "illegal move: {mv}");
	}

	#[test]
	fn rollout_plays_full_game() {
		let mut rng = SmallRng::seed_from_u64(42);
		let mut state: GameState<5> = GameState::new(GameConfig::default(), &mut rng);
		let mut bot = Rollout {};

		while state.outcome().is_none() {
			let mv = bot.choose_move(&state);
			state.play(mv).expect("illegal move");
		}

		assert!(state.outcome().is_some());
	}

	#[test]
	fn modal_greed_early_game() {
		// Early game: center card only, all lines unfinished → mode 1.
		// Weakest lines (score 0) get priority. Card 5 has highest face value delta on empty lines.
		let mut grid = [[None; 5]; 5];
		grid[2][2] = Some(3);
		let state = make_state(grid, hand(&[(3, 2), (1, 1), (5, 1)]), Player::A);
		let analysis = LineAnalysis::new(&state);
		assert!(analysis.finished_min.is_none());
		let mv = modal_greed(&state, &analysis);
		assert_eq!(mv.card, CardValue(5));
	}

	#[test]
	fn averse_midgame() {
		let grid = [
			[None, None, Some(1), Some(1), Some(0)],
			[None, Some(2), None, Some(3), None],
			[Some(4), None, None, None, None],
			[None, Some(2), None, None, Some(0)],
			[Some(4), Some(4), Some(4), Some(0), Some(0)],
		];
		// Both players need hands for projection to complete the game.
		let h = hand(&[(0, 1), (1, 2), (3, 1), (5, 2)]);
		let mut board = Board::<5>::default();
		for row in 0..5u8 {
			for col in 0..5u8 {
				if let Some(v) = grid[row as usize][col as usize] {
					board.set(Pos { row, col }, v);
				}
			}
		}
		let state = GameState {
			board,
			hands: [h, hand(&[(0, 1), (2, 2), (4, 1), (5, 2)])],
			turn: Player::B,
			config: GameConfig::default(),
		};
		let mv = Averse::builder().fuel(500).build().choose(&state);
		assert_snapshot!(format!("card={} pos=({},{})", mv.card.0, mv.pos.row, mv.pos.col));
	}
}
