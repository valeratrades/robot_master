use board_game::board::Board as _;
use robot_master_core::{
	board::{EMPTY, Pos},
	cards::{CardValue, MAX_BOARD_SIZE},
	game::{GameState, Move, Player},
	scoring::{LineCounts, line_counts, score_line},
};
use v_utils::macros::CompactFormatNamed;

use crate::player::Bot;

/// Sadist player: minimizes the opponent's maximum potential score.
#[derive(Clone, CompactFormatNamed, Debug, Default, Eq, PartialEq)]
pub struct Sadist {}

impl<const N: usize> Bot<N> for Sadist
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let opponent = game.turn.other();

		let mut best_move: Option<Move> = None;
		let mut best_score: Option<(u16, u8, u8, u8)> = None; // (opp_potential, card, row, col)

		for m in game.valid_moves() {
			let next = game.clone_and_play(m).expect("valid_moves produced illegal move");
			let opp_potential = score_max_potential::<N>(&next, opponent);

			let key = (opp_potential, m.card.0, m.pos.row, m.pos.col);
			if best_score.is_none_or(|bs| key < bs) {
				best_score = Some(key);
				best_move = Some(m);
			}
		}

		best_move.expect("no valid move found")
	}
}

/// Maximum score the given player could achieve on their best line,
/// considering all possible completions with remaining cards.
fn score_max_potential<const N: usize>(game: &GameState<N>, player: Player) -> u16
where
	[(); N * N]:,
	[(); N + 1]:, {
	let remaining = remaining_cards(game, player);
	let mut best = 0u16;

	for i in 0..N {
		let counts = line_counts(&game.board.line(player, i));
		let empty_slots = N - counts.iter().map(|&c| c as usize).sum::<usize>();

		let mut scores = Vec::default();
		complete_and_score(&mut counts.clone(), &mut remaining.clone(), &mut scores, empty_slots, 0, N);

		if let Some(&max) = scores.iter().max() {
			best = best.max(max);
		}
	}

	best
}

/// Recursively enumerate all multiset completions of a line and collect their scores.
///
/// Iterates over card values (not slots) to avoid permutation explosion.
/// Each distinct multiset of additions is visited exactly once.
fn complete_and_score(counts: &mut LineCounts, remaining: &mut LineCounts, scores: &mut Vec<u16>, empty_slots: usize, card_index: usize, max_card: usize) {
	if empty_slots == 0 {
		scores.push(score_line(counts));
		return;
	}

	if card_index > max_card {
		return;
	}

	let available = remaining[card_index] as usize;
	let can_place = available.min(empty_slots);

	// Try placing 0..=can_place copies of card_index, then recurse to next card value.
	for n in 0..=can_place {
		complete_and_score(counts, remaining, scores, empty_slots - n, card_index + 1, max_card);
		if n < can_place {
			counts[card_index] += 1;
			remaining[card_index] -= 1;
		}
	}

	// Undo all placed copies.
	let placed = can_place;
	counts[card_index] -= placed as u8;
	remaining[card_index] += placed as u8;
}

/// Count remaining copies of each card value available to the opponent of `player`.
/// Subtracts cards already on the board and cards in `player`'s own hand (which
/// the opponent cannot hold).
fn remaining_cards<const N: usize>(game: &GameState<N>, player: Player) -> LineCounts
where
	[(); N * N]:,
	[(); N + 1]:, {
	let mut played = [0u8; MAX_BOARD_SIZE + 1];
	for row in 0..N {
		for col in 0..N {
			let cell = game.board.get(Pos { row: row as u8, col: col as u8 });
			if cell != EMPTY {
				played[cell as usize] += 1;
			}
		}
	}
	let hands = game.hands().expect("sadist does not support hidden hands");
	let own_hand = hands[player.index() as usize];
	let mut remaining = [0u8; MAX_BOARD_SIZE + 1];
	for v in 0..=N {
		let in_hand = own_hand.count(CardValue(v as u8));
		remaining[v] = (N as u8 + 1) - played[v] - in_hand;
	}
	remaining
}

#[cfg(test)]
mod tests {
	use insta::assert_snapshot;
	use robot_master_core::game::Player;

	use super::{super::test_utils::fixtures::*, *};

	#[test]
	fn tous_les_scores_possibles_no_duplicates() {
		let mut counts = [0u8; MAX_BOARD_SIZE + 1];
		let mut remaining = [0u8; MAX_BOARD_SIZE + 1];
		remaining[1] = 2;
		remaining[2] = 2;
		let mut scores = Vec::default();
		complete_and_score(&mut counts, &mut remaining, &mut scores, 2, 0, 2);
		scores.sort();
		assert_eq!(scores.len(), scores.iter().collect::<std::collections::HashSet<_>>().len());
		assert_snapshot!(format!("{scores:?}"), @"[3, 10, 20]");
	}

	#[test]
	fn tous_les_scores_possibles_already_complete() {
		let mut counts = [0u8; MAX_BOARD_SIZE + 1];
		counts[1] = 2;
		counts[2] = 3;
		let mut remaining = [0u8; MAX_BOARD_SIZE + 1];
		remaining[0] = 1;
		remaining[1] = 1;
		remaining[2] = 1;
		let mut scores = Vec::default();
		complete_and_score(&mut counts, &mut remaining, &mut scores, 0, 0, 2);
		assert_snapshot!(format!("{scores:?}"), @"[110]");
	}

	#[test]
	fn agressif_midgame() {
		let state = make_state(board_midgame(), hand(&[(0, 1), (1, 2), (3, 1), (5, 2)]), Player::B);
		let m = Sadist {}.choose_move(&state);
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
		move: card=0 pos=(0,1)
		");
	}

	#[test]
	fn agressif_game_rollout() {
		assert_snapshot!(run_midgame_rollout(&mut Sadist {}), @"
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 |+0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |+0 | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   | 0 | 1 | 1 | 0 |
		(1,_)   |+1 | 2 |   | 3 |   |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 |   | 3 |+1 |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |+2 | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 |   | 3 | 1 |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 2 | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 |+3 | 3 | 1 |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=A
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 2 | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 | 3 | 3 | 1 |
		(2,_)   | 4 | 0 |+5 |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		---
		turn=B
		-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 2 | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 | 3 | 3 | 1 |
		(2,_)   | 4 | 0 | 5 |   |+5 |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		-----------------------------
		");
	}
}
