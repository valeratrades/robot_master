use robot_master_core::{
	board::Pos,
	cards::CardValue,
	game::{GameState, Move, scores_rows},
	scoring::{line_counts, score_delta},
};
use ustr::{Ustr, ustr};

use crate::player::Bot;

/// Greedy player: maximizes immediate score delta on own lines.
///
/// Faithful port of `choix_carte_greedy` from `py_src/IA/g_greedy.py`.
///
/// Algorithm:
/// 1. For each of the player's lines, extract current card counts.
/// 2. For each playable position, determine which line it affects.
/// 3. Pick one representative position per line (first found).
/// 4. For each line with a position, score every card in hand by `score_delta`.
/// 5. Return the (card, position) with the globally best delta.
///    Tiebreak: highest delta, then lowest card value.
///
/// Limitation: treats each line independently, no lookahead.
#[derive(Clone)]
pub struct GreedyPlayer;

impl<const N: usize> Bot<N> for GreedyPlayer
where
	[(); N * N]:,
{
	fn id(&self) -> Ustr {
		ustr("greedy")
	}

	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let turn = game.turn;
		let hand = &game.hands[turn.index() as usize];
		let board = &game.board;

		// Compute line counts for each of the player's lines.
		let lines: Vec<_> = (0..N).map(|i| line_counts(&board.line(turn, i))).collect();

		// Map each playable position to its line index, keep first representative per line.
		let mut line_pos: Vec<Option<Pos>> = vec![None; N];
		for pos in board.valid_placements() {
			let line_idx = if scores_rows(turn) { pos.row as usize } else { pos.col as usize };
			if line_pos[line_idx].is_none() {
				line_pos[line_idx] = Some(pos);
			}
		}

		// Find best (card, position) across all lines.
		let mut best_delta: Option<i16> = None;
		let mut best_card: Option<CardValue> = None;
		let mut best_pos: Option<Pos> = None;

		for (i, pos) in line_pos.iter().enumerate() {
			let Some(pos) = pos else { continue };
			let counts = &lines[i];

			for card in hand.iter_playable() {
				let delta = score_delta(counts, card);
				let dominated = best_delta.is_some_and(|bd| delta < bd || (delta == bd && card >= best_card.unwrap()));
				if !dominated {
					best_delta = Some(delta);
					best_card = Some(card);
					best_pos = Some(*pos);
				}
			}
		}

		Move {
			pos: best_pos.expect("no valid move found"),
			card: best_card.expect("no valid move found"),
		}
	}
}

#[cfg(test)]
mod tests {
	use insta::assert_snapshot;
	use robot_master_core::{
		board::Board,
		cards::Hand,
		game::{GameConfig, Player},
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

	fn board_one_card() -> [[Option<u8>; 5]; 5] {
		let mut g = [[None; 5]; 5];
		g[2][2] = Some(3);
		g
	}

	fn board_midgame() -> [[Option<u8>; 5]; 5] {
		[
			[None, None, Some(1), Some(1), Some(0)],
			[None, Some(2), None, Some(3), None],
			[Some(4), None, None, None, None],
			[None, Some(2), None, None, Some(0)],
			[Some(4), Some(4), Some(4), Some(0), Some(0)],
		]
	}

	#[test]
	fn picks_highest_delta_odd_player() {
		// Row 2 already has a 3; playing another 3 gives delta=27 (9*3) vs delta=1 for card 1.
		let state = make_state(board_one_card(), hand(&[(1, 1), (3, 2)]), Player::B);
		let m = GreedyPlayer.choose_move(&state);
		assert_eq!(m.card, CardValue(3));
		assert_eq!(m.pos.row, 2);
		assert_snapshot!(format!("{}\nmove: card={} pos=({},{})", state.board, m.card.0, m.pos.row, m.pos.col), @"
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   |   |   |   |
		(1,_)   |   |   |   |   |   |
		(2,_)   |   |   | 3 |   |   |
		(3,_)   |   |   |   |   |   |
		(4,_)   |   |   |   |   |   |
		-----------------------------
		move: card=3 pos=(2,1)
		");
	}

	#[test]
	fn picks_highest_delta_even_player() {
		// Col 2 already has a 3; even player scores columns.
		let state = make_state(board_one_card(), hand(&[(1, 1), (3, 2)]), Player::A);
		let m = GreedyPlayer.choose_move(&state);
		assert_eq!(m.card, CardValue(3));
		assert_eq!(m.pos.col, 2);
	}

	#[test]
	fn midgame_odd_player() {
		let state = make_state(board_midgame(), hand(&[(0, 1), (1, 2), (3, 1), (5, 2)]), Player::B);
		let m = GreedyPlayer.choose_move(&state);
		assert_snapshot!(format!("{}\nmove: card={} pos=({},{})", state.board, m.card.0, m.pos.row, m.pos.col), @"
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		move: card=1 pos=(0,1)
		");
	}

	#[test]
	fn midgame_even_player() {
		let state = make_state(board_midgame(), hand(&[(0, 1), (1, 2), (3, 1), (5, 2)]), Player::A);
		let m = GreedyPlayer.choose_move(&state);
		assert_snapshot!(format!("card={} pos=({},{})", m.card.0, m.pos.row, m.pos.col), @"card=3 pos=(2,3)");
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
			let m = GreedyPlayer.choose_move(&state);
			moves.push(format!("board={board} turn={turn:?} card={} pos=({},{})", m.card.0, m.pos.row, m.pos.col));
			board.set(m.pos, m.card.0);
			hand_counts[m.card.0 as usize] -= 1;
			if hand_counts.iter().all(|&c| c == 0) {
				break;
			}
		}

		assert_snapshot!(moves.join("\n---\n"), @"
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=A card=2 pos=(0,1)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   | 2 | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=B card=1 pos=(0,0)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=A card=3 pos=(2,3)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=B card=5 pos=(1,0)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   | 5 | 2 |   | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=A card=5 pos=(3,0)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   | 5 | 2 |   | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   | 5 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=B card=1 pos=(1,2)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   | 5 | 2 | 1 | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   | 5 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=A card=0 pos=(2,1)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   | 5 | 2 | 1 | 3 |   |
		(2,_)   | 4 | 0 |   | 3 |   |
		(3,_)   | 5 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=B card=0 pos=(1,4)
		");
	}
}
