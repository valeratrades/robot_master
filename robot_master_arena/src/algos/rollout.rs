use board_game::board::Board as _;
use bon::Builder;
use robot_master_core::{
	board::{Board, EMPTY},
	game::{GameState, Move, Player, scores_rows},
	scoring::{line_counts, score_delta, score_line},
};
use v_utils::macros::CompactFormatNamed;

use super::greedy::Greedy;
use crate::player::Bot;

/// Three-mode bot:
/// 1. No finished lines → pure greedy (maximize immediate score_delta across all lines)
/// 2. Some finished lines, unfinished lines below finished_min → target those weak lines
/// 3. No improvable lines → averse (harass opponent)
#[derive(Clone, CompactFormatNamed, Debug)]
pub struct Rollout {}

impl<const N: usize> Bot<N> for Rollout
where
	[(); N * N]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let analysis = LineAnalysis::new(game);

		match analysis.mode() {
			Mode::Greedy => Greedy {}.choose_move(game),
			Mode::TargetWeak => target_weak(game, &analysis),
			Mode::Averse => Averse::builder().fuel(500).build().choose(game),
		}
	}
}

enum Mode {
	/// No finished lines — play pure greedy.
	Greedy,
	/// At least one finished line, but unfinished lines exist below finished_min — target those.
	TargetWeak,
	/// All unfinished lines >= finished_min — can't improve, harass opponent.
	Averse,
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

	fn mode(&self) -> Mode {
		let Some(fmin) = self.finished_min else {
			return Mode::Greedy;
		};
		if self.lines.iter().any(|&(_, score, finished)| !finished && score < fmin) {
			Mode::TargetWeak
		} else {
			Mode::Averse
		}
	}
}

/// Mode 2: maximize score_delta on unfinished lines below finished_min.
fn target_weak<const N: usize>(game: &GameState<N>, analysis: &LineAnalysis) -> Move
where
	[(); N * N]:, {
	let fmin = analysis.finished_min.expect("target_weak called without finished lines");
	let player = game.turn;
	let hand = &game.hands[player.index() as usize];
	let board = &game.board;

	let target_lines: Vec<usize> = analysis.lines.iter().filter(|l| !l.2 && l.1 < fmin).map(|l| l.0).collect();

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

	// If no target line has a playable position, fall back to greedy.
	best.map(|b| b.1).unwrap_or_else(|| Greedy {}.choose_move(game))
}

/// Bounded sadist: minimizes opponent's max potential, but uses greedy for opponent
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

/// Project what score the opponent could reach if they play greedy from here.
/// Simulates until the game ends, then returns opponent's final min-line score.
fn project_opponent_score<const N: usize>(game: &GameState<N>, opponent: Player) -> u16
where
	[(); N * N]:, {
	let mut sim = game.clone();
	let mut greedy = Greedy {};

	while sim.outcome().is_none() {
		let mv = greedy.choose_move(&sim);
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
	fn early_game_matches_greedy() {
		let mut grid = [[None; 5]; 5];
		grid[2][2] = Some(3);
		let state = make_state(grid, hand(&[(3, 2), (1, 1), (5, 1)]), Player::A);
		let analysis = LineAnalysis::new(&state);
		assert!(matches!(analysis.mode(), Mode::Greedy));
		assert_eq!(Rollout {}.choose_move(&state), Greedy {}.choose_move(&state));
	}

	#[test]
	fn game_rollout() {
		let mut board = Board::<5>::default();
		for (row, col, v) in [
			(0u8, 2u8, 1u8),
			(0, 3, 1),
			(0, 4, 0),
			(1, 1, 2),
			(1, 3, 3),
			(2, 0, 4),
			(3, 1, 2),
			(3, 4, 0),
			(4, 0, 4),
			(4, 1, 4),
			(4, 2, 4),
			(4, 3, 0),
			(4, 4, 0),
		] {
			board.set(Pos { row, col }, v);
		}

		let mut hand_counts = [0u8; 6];
		hand_counts[0] = 2;
		hand_counts[1] = 2;
		hand_counts[2] = 1;
		hand_counts[3] = 1;
		hand_counts[5] = 2;

		fn make_hand_from_counts(counts: &[u8; 6]) -> Hand {
			let mut h = Hand::default();
			for (v, &n) in counts.iter().enumerate() {
				for _ in 0..n {
					h.put(CardValue(v as u8));
				}
			}
			h
		}

		let mut moves: Vec<String> = Vec::new();
		let turns = [Player::A, Player::B];
		let mut bot = Rollout {};

		for turn_idx in 0..10usize {
			let turn = turns[turn_idx % 2];
			let h = make_hand_from_counts(&hand_counts);
			if h.is_empty() {
				break;
			}
			let state = GameState {
				board,
				hands: match turn {
					Player::A => [h, Hand::default()],
					Player::B => [Hand::default(), h],
				},
				turn,
				config: GameConfig::default(),
			};
			let m = bot.choose_move(&state);
			let prev = board;
			board.set(m.pos, m.card.0);
			moves.push(format!("turn={turn:?}\n{}", board.display_diff(&prev)));
			hand_counts[m.card.0 as usize] -= 1;
			if hand_counts.iter().all(|&c| c == 0) {
				break;
			}
		}

		assert_snapshot!(moves.join("\n---\n"));
	}
}
