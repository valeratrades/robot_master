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
/// largest. This raises the weakest line first - matching the project spec's
/// `choix_carte_greedy` which maximizes `score_complet_joueuse`.
///
/// Tiebreak: lexicographically largest score vector, then lowest card value,
/// then lowest (posL, posC).
#[derive(Clone, CompactFormatNamed, Debug, Default, Eq, PartialEq)]
pub struct GreedyForScore {}

impl<const N: usize> Bot<N> for GreedyForScore
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let turn = game.turn;
		let hands = game.hands().expect("greedy_min does not support hidden hands");
		let hand = &hands[turn.index() as usize];
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

	use super::{super::test_utils::fixtures::*, *};

	#[test]
	fn game_rollout() {
		assert_snapshot!(run_midgame_rollout(&mut GreedyForScore {}), @"
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
