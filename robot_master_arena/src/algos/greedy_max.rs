use robot_master_core::{
	board::Pos,
	cards::CardValue,
	game::{GameState, Move, scores_rows},
	scoring::{line_counts, score_delta},
};
use v_utils::macros::CompactFormatNamed;

use crate::player::Bot;

/// Greedy player: maximizes immediate score delta on the single best line.
///
/// Picks the (card, position) that gives the highest `score_delta` on whichever
/// line it lands in. This tends to chase big scores (pairs/triples) on one line
/// while potentially leaving other lines at zero.
///
/// Tiebreak: highest delta, then lowest card value.
#[derive(Clone, CompactFormatNamed, Debug, Default, Eq, PartialEq)]
pub struct GreedyForNumber {}

impl<const N: usize> Bot<N> for GreedyForNumber
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let turn = game.turn;
		let hand = &game.hands[turn.index() as usize];
		let board = &game.board;

		let lines: Vec<_> = (0..N).map(|i| line_counts(&board.line(turn, i))).collect();

		let mut line_pos: Vec<Option<Pos>> = vec![None; N];
		for pos in board.valid_placements() {
			let line_idx = if scores_rows(turn) { pos.row as usize } else { pos.col as usize };
			if line_pos[line_idx].is_none() {
				line_pos[line_idx] = Some(pos);
			}
		}

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
	use robot_master_core::{cards::CardValue, game::Player};

	use super::{super::test_utils::fixtures::*, *};

	#[test]
	fn picks_highest_delta_odd_player() {
		let state = make_state(board_one_card(), hand(&[(1, 1), (3, 2)]), Player::B);
		let m = GreedyForNumber {}.choose_move(&state);
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
	fn game_rollout() {
		assert_snapshot!(run_midgame_rollout(&mut GreedyForNumber {}), @"
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |+2 | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |+1 | 2 | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |   |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
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
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   |+5 | 2 |   | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   | 5 | 2 |   | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   |+5 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   | 5 | 2 |+1 | 3 |   |
		(2,_)   | 4 |   |   | 3 |   |
		(3,_)   | 5 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   | 5 | 2 | 1 | 3 |   |
		(2,_)   | 4 |+0 |   | 3 |   |
		(3,_)   | 5 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 1 | 2 | 1 | 1 | 0 |
		(1,_)   | 5 | 2 | 1 | 3 |+0 |
		(2,_)   | 4 | 0 |   | 3 |   |
		(3,_)   | 5 | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		");
	}
}
