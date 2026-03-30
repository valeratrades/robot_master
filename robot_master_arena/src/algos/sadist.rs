use robot_master_core::{
	board::{EMPTY, Pos},
	cards::MAX_CARD_VALUE,
	game::{GameState, Move, Player},
	scoring::{LineCounts, line_counts, score_line},
};
use ustr::{Ustr, ustr};

use crate::player::Bot;

/// Sadist player: minimizes the opponent's maximum potential score.
///
/// Faithful port of `choix_carte_agressif` from `py_src/IA/h_agressif.py`.
///
/// Algorithm:
/// 1. For each valid (position, card) pair:
///    a. Simulate the move.
///    b. For each of the opponent's lines, enumerate all possible completions using remaining cards, take max score.
///    c. The opponent's "max potential" = max across all their lines.
/// 2. Pick the move that minimizes this opponent max potential.
/// 3. Tiebreak: (score, card, row, col) — lower is better lexicographically.
pub struct SadistPlayer;

impl<const N: usize> Bot<N> for SadistPlayer
where
	[(); N * N]:,
{
	fn id(&self) -> Ustr {
		ustr("sadist")
	}

	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		let opponent = game.turn.other();

		let mut best_move: Option<Move> = None;
		let mut best_score: Option<(u16, u8, u8, u8)> = None; // (opp_potential, card, row, col)

		for m in game.valid_moves() {
			let next = game.apply_move(m).expect("valid_moves produced illegal move");
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
	[(); N * N]:, {
	let remaining = remaining_cards(game);
	let mut best = 0u16;

	for i in 0..N {
		let counts = line_counts(&game.board.line(player, i));
		let empty_slots = N - counts.iter().map(|&c| c as usize).sum::<usize>();

		let mut scores = Vec::new();
		complete_and_score(&mut counts.clone(), &mut remaining.clone(), &mut scores, empty_slots, 0, game.config.max_card as usize);

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

/// Count remaining copies of each card value (not yet on the board).
fn remaining_cards<const N: usize>(game: &GameState<N>) -> LineCounts
where
	[(); N * N]:, {
	let mut played = [0u8; MAX_CARD_VALUE + 1];
	for row in 0..N {
		for col in 0..N {
			let cell = game.board.get(Pos { row: row as u8, col: col as u8 });
			if cell != EMPTY {
				played[cell as usize] += 1;
			}
		}
	}
	let mut remaining = [0u8; MAX_CARD_VALUE + 1];
	for v in 0..=game.config.max_card as usize {
		remaining[v] = game.config.nb_c - played[v];
	}
	remaining
}

#[cfg(test)]
mod tests {
	use insta::assert_snapshot;
	use robot_master_core::{
		board::Board,
		cards::{CardValue, Hand},
		game::GameConfig,
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
	fn tous_les_scores_possibles_no_duplicates() {
		let mut counts = [0u8; MAX_CARD_VALUE + 1];
		let mut remaining = [0u8; MAX_CARD_VALUE + 1];
		remaining[1] = 2;
		remaining[2] = 2;
		let mut scores = Vec::new();
		complete_and_score(&mut counts, &mut remaining, &mut scores, 2, 0, 2);
		scores.sort();
		assert_eq!(scores.len(), scores.iter().collect::<std::collections::HashSet<_>>().len());
		assert_snapshot!(format!("{scores:?}"), @"[3, 10, 20]");
	}

	#[test]
	fn tous_les_scores_possibles_already_complete() {
		let mut counts = [0u8; MAX_CARD_VALUE + 1];
		counts[1] = 2;
		counts[2] = 3;
		let mut remaining = [0u8; MAX_CARD_VALUE + 1];
		remaining[0] = 1;
		remaining[1] = 1;
		remaining[2] = 1;
		let mut scores = Vec::new();
		complete_and_score(&mut counts, &mut remaining, &mut scores, 0, 0, 2);
		assert_snapshot!(format!("{scores:?}"), @"[110]");
	}

	#[test]
	fn agressif_midgame() {
		let state = make_state(board_midgame(), hand(&[(0, 1), (1, 2), (3, 1), (5, 2)]), Player::B);
		let m = SadistPlayer.choose_move(&state);
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
			let m = SadistPlayer.choose_move(&state);
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
		----------------------------- turn=A card=0 pos=(2,1)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   |   | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=B card=0 pos=(0,1)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   | 0 | 1 | 1 | 0 |
		(1,_)   |   | 2 |   | 3 |   |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=A card=1 pos=(1,0)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 |   | 3 |   |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=B card=1 pos=(1,4)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   |   | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 |   | 3 | 1 |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=A card=2 pos=(0,0)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 2 | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 |   | 3 | 1 |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=B card=3 pos=(1,2)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 2 | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 | 3 | 3 | 1 |
		(2,_)   | 4 | 0 |   |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=A card=5 pos=(2,2)
		---
		board=-----------------------------
		          0   1   2   3   4
		-----------------------------
		(0,_)   | 2 | 0 | 1 | 1 | 0 |
		(1,_)   | 1 | 2 | 3 | 3 | 1 |
		(2,_)   | 4 | 0 | 5 |   |   |
		(3,_)   |   | 2 |   |   | 0 |
		(4,_)   | 4 | 4 | 4 | 0 | 0 |
		----------------------------- turn=B card=5 pos=(2,4)
		");
	}
}
