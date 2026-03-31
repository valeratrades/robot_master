#[cfg(test)]
pub(super) mod fixtures {
	use robot_master_core::{
		board::{Board, Pos},
		cards::{CardValue, Hand},
		game::{GameConfig, GameState, Player},
	};

	use crate::player::Bot;

	pub fn make_state(grid: [[Option<u8>; 5]; 5], hand: Hand, turn: Player) -> GameState<5> {
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

	pub fn hand(pairs: &[(u8, u8)]) -> Hand {
		let mut h = Hand::default();
		for &(v, n) in pairs {
			for _ in 0..n {
				h.put(CardValue(v));
			}
		}
		h
	}

	pub fn board_one_card() -> [[Option<u8>; 5]; 5] {
		let mut g = [[None; 5]; 5];
		g[2][2] = Some(3);
		g
	}

	pub fn board_midgame() -> [[Option<u8>; 5]; 5] {
		[
			[None, None, Some(1), Some(1), Some(0)],
			[None, Some(2), None, Some(3), None],
			[Some(4), None, None, None, None],
			[None, Some(2), None, None, Some(0)],
			[Some(4), Some(4), Some(4), Some(0), Some(0)],
		]
	}

	/// Run a deterministic game rollout from a fixed midgame position, returning
	/// the display-diff string for each move. The only varying part is which bot
	/// makes decisions.
	pub fn run_midgame_rollout(bot: &mut dyn Bot<5>) -> String {
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

		let mut moves: Vec<String> = Vec::new();
		let turns = [Player::A, Player::B];

		for turn_idx in 0..10usize {
			let turn = turns[turn_idx % 2];
			let h = hand_from_counts(&hand_counts);
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

		moves.join("\n---\n")
	}

	fn hand_from_counts(counts: &[u8; 6]) -> Hand {
		let mut h = Hand::default();
		for (v, &n) in counts.iter().enumerate() {
			for _ in 0..n {
				h.put(CardValue(v as u8));
			}
		}
		h
	}
}
