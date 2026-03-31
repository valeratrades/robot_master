use robot_master_core::{
	board::Pos,
	cards::CardValue,
	game::{GameState, Move},
	scoring::{line_counts, score_line},
};
use v_utils::macros::CompactFormatNamed;

use crate::player::Bot;

/// Greedy player: maximizes `score_complet` (sorted line-score vector).
///
/// For each candidate move, simulates placing the card, computes every line's
/// score, sorts them, and picks the move whose sorted vector is lexicographically
/// largest. This raises the weakest line first — matching the project spec's
/// `choix_carte_greedy` which maximizes `score_complet_joueuse`.
///
/// Tiebreak: lexicographically largest score vector, then lowest card value,
/// then lowest (posL, posC).
#[derive(Clone, CompactFormatNamed, Debug, Default, Eq, PartialEq)]
pub struct GreedyMin {}

impl<const N: usize> Bot<N> for GreedyMin
where
	[(); N * N]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let turn = game.turn;
		let hand = &game.hands[turn.index() as usize];
		let board = &game.board;

		let mut best_scores: Option<Vec<u16>> = None;
		let mut best_card: Option<CardValue> = None;
		let mut best_pos: Option<Pos> = None;

		for pos in board.valid_placements() {
			for card in hand.iter_playable() {
				let mut b = *board;
				b.set(pos, card.0);

				let mut scores: Vec<u16> = (0..N).map(|i| score_line(&line_counts(&b.line(turn, i)))).collect();
				scores.sort();

				let dominated = best_scores
					.as_ref()
					.is_some_and(|bs| scores < *bs || (scores == *bs && (card > best_card.unwrap() || (card == best_card.unwrap() && pos >= best_pos.unwrap()))));
				if !dominated {
					best_scores = Some(scores);
					best_card = Some(card);
					best_pos = Some(pos);
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
	fn picks_card3_odd_player() {
		let state = make_state(board_one_card(), hand(&[(1, 1), (3, 2)]), Player::B);
		let m = GreedyMin {}.choose_move(&state);
		assert_eq!(m.card, CardValue(3));
		assert_snapshot!(format!("card={} pos=({},{})", m.card.0, m.pos.row, m.pos.col), @"card=3 pos=(1,2)");
	}

	#[test]
	fn picks_card3_even_player() {
		let state = make_state(board_one_card(), hand(&[(1, 1), (3, 2)]), Player::A);
		let m = GreedyMin {}.choose_move(&state);
		assert_eq!(m.card, CardValue(3));
		assert_snapshot!(format!("card={} pos=({},{})", m.card.0, m.pos.row, m.pos.col), @"card=3 pos=(2,1)");
	}

	#[test]
	fn midgame_odd_player() {
		let state = make_state(board_midgame(), hand(&[(0, 1), (1, 2), (3, 1), (5, 2)]), Player::B);
		let m = GreedyMin {}.choose_move(&state);
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
		move: card=5 pos=(3,0)
		");
	}

	#[test]
	fn midgame_even_player() {
		let state = make_state(board_midgame(), hand(&[(0, 1), (1, 2), (3, 1), (5, 2)]), Player::A);
		let m = GreedyMin {}.choose_move(&state);
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
			let m = GreedyMin {}.choose_move(&state);
			let prev = board;
			board.set(m.pos, m.card.0);
			moves.push(format!("turn={turn:?}\n{}", board.display_diff(&prev)));
			hand_counts[m.card.0 as usize] -= 1;
			if hand_counts.iter().all(|&c| c == 0) {
				break;
			}
		}

		assert_snapshot!(moves.join("\n---\n"), @"
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   |+3 |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   |+2 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   |   | 2 |+1 | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   | 2 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   |+1 | 2 | 1 | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   | 2 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   | 1 | 2 | 1 | 3 |   |
		(2,_)   | 4 |   |+5 | 3 |   |
		(3,_)   | 2 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |+5 |   | 1 | 1 | 0 |
		(1,_)   | 1 | 2 | 1 | 3 |   |
		(2,_)   | 4 |   | 5 | 3 |   |
		(3,_)   | 2 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 5 |+0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 | 1 | 3 |   |
		(2,_)   | 4 |   | 5 | 3 |   |
		(3,_)   | 2 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 5 | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 | 1 | 3 |+0 |
		(2,_)   | 4 |   | 5 | 3 |   |
		(3,_)   | 2 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		");
	}
}
