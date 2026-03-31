use robot_master_core::{
	board::EMPTY,
	cards::MAX_CARD_VALUE,
	game::{GameState, Player},
};

/// Number of input channels — must match `IN_CHANNELS` in `training/model_resnet.py`.
pub const IN_CHANNELS: usize = 33;

/// Encode `GameState<N>` into `IN_CHANNELS * N * N` f32 values, channel-first (CHW).
///
/// Layout (matches `model_resnet.py::encode_state`):
///   [0:6]   card value presence  — plane `v` is 1.0 where board cell == v
///   [6]     empty cells          — 1.0 where cell == EMPTY
///   [7]     playable cells       — 1.0 where empty and has ≥1 occupied neighbour
///   [8:20]  current player hand  — 2 planes per value: ≥1 copy, ≥2 copies (broadcast)
///   [20:32] opponent hand        — same encoding
///   [32]    turn indicator       — 1.0 if current player is Player::A
pub fn encode_planes<const N: usize>(state: &GameState<N>) -> Vec<f32>
where
	[(); N * N]:, {
	let n2 = N * N;
	let mut planes = vec![0.0f32; IN_CHANNELS * n2];

	let set = |planes: &mut Vec<f32>, ch: usize, row: usize, col: usize, v: f32| {
		planes[ch * n2 + row * N + col] = v;
	};

	for row in 0..N {
		for col in 0..N {
			let cell = state.board.get(robot_master_core::board::Pos { row: row as u8, col: col as u8 });
			if cell == EMPTY {
				set(&mut planes, 6, row, col, 1.0);
			} else {
				set(&mut planes, cell as usize, row, col, 1.0);
			}
		}
	}

	// playable: empty + has occupied neighbour — reuse Board::is_playable
	for row in 0..N {
		for col in 0..N {
			let pos = robot_master_core::board::Pos { row: row as u8, col: col as u8 };
			if state.board.is_playable(pos) {
				set(&mut planes, 7, row, col, 1.0);
			}
		}
	}

	let current_idx = state.turn.index() as usize;
	let opponent_idx = 1 - current_idx;
	let hand_cur = &state.hands[current_idx];
	let hand_opp = &state.hands[opponent_idx];

	for v in 0..=MAX_CARD_VALUE {
		let card = robot_master_core::cards::CardValue(v as u8);
		let cnt_cur = hand_cur.count(card);
		let cnt_opp = hand_opp.count(card);

		// broadcast: fill entire plane with 1.0
		let fill_plane = |planes: &mut Vec<f32>, ch: usize| {
			let start = ch * n2;
			planes[start..start + n2].fill(1.0);
		};

		if cnt_cur >= 1 {
			fill_plane(&mut planes, 8 + v * 2);
		}
		if cnt_cur >= 2 {
			fill_plane(&mut planes, 8 + v * 2 + 1);
		}
		if cnt_opp >= 1 {
			fill_plane(&mut planes, 20 + v * 2);
		}
		if cnt_opp >= 2 {
			fill_plane(&mut planes, 20 + v * 2 + 1);
		}
	}

	if state.turn == Player::A {
		let start = 32 * n2;
		planes[start..start + n2].fill(1.0);
	}

	planes
}

/// Number of possible actions: 6 card values × N² positions.
pub const fn action_size(n: usize) -> usize {
	(MAX_CARD_VALUE + 1) * n * n
}

/// Map `(card_value, row, col)` to a flat policy index.
///
/// Matches `AllMoves` iterator order in `game.rs` and Python's policy layout.
pub fn action_index(card: u8, row: usize, col: usize, n: usize) -> usize {
	card as usize * n * n + row * n + col
}

/// Serialize one training sample to raw bytes (little-endian f32s).
///
/// Layout: `state_planes || policy || [value]`
/// — matches `SelfPlayDataset` in `training/train.py`.
pub fn encode_sample(state_planes: &[f32], policy: &[f32], value: f32) -> Vec<u8> {
	let total = state_planes.len() + policy.len() + 1;
	let mut out = Vec::with_capacity(total * 4);
	for &f in state_planes.iter().chain(policy.iter()).chain(std::iter::once(&value)) {
		out.extend_from_slice(&f.to_le_bytes());
	}
	out
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng};
	use robot_master_core::game::{GameConfig, GameState};

	use super::*;

	fn state5() -> GameState<5> {
		let mut rng = SmallRng::seed_from_u64(42);
		GameState::new(GameConfig::default(), &mut rng)
	}

	#[test]
	fn planes_shape() {
		let state = state5();
		let planes = encode_planes(&state);
		assert_eq!(planes.len(), IN_CHANNELS * 5 * 5);
	}

	#[test]
	fn planes_center_card() {
		let state = state5();
		// Center is (2,2). Whatever card is there should have its plane set to 1.0.
		let center_cell = state.board.get(robot_master_core::board::Pos { row: 2, col: 2 });
		assert_ne!(center_cell, EMPTY);
		let planes = encode_planes(&state);
		assert_eq!(planes[center_cell as usize * 25 + 2 * 5 + 2], 1.0, "card-value plane at center");
		assert_eq!(planes[6 * 25 + 2 * 5 + 2], 0.0, "empty plane not set at center");
	}

	#[test]
	fn planes_playable_adjacent_to_center() {
		let state = state5();
		let planes = encode_planes(&state);
		// (2,1) is adjacent to center (2,2) and should be playable at game start
		assert_eq!(planes[7 * 25 + 2 * 5 + 1], 1.0, "adjacent to center is playable");
		// (0,0) is not adjacent to anything occupied
		assert_eq!(planes[7 * 25 + 0 * 5 + 0], 0.0, "corner is not playable");
	}

	#[test]
	fn turn_indicator_player_a() {
		let state = state5(); // starts as Player::A
		let planes = encode_planes(&state);
		assert_eq!(planes[32 * 25], 1.0, "player A indicator");
	}

	#[test]
	fn encode_sample_byte_length() {
		let n = 5usize;
		let state_planes = vec![0.0f32; IN_CHANNELS * n * n];
		let policy = vec![0.0f32; action_size(n)];
		let bytes = encode_sample(&state_planes, &policy, 1.0);
		let expected = (IN_CHANNELS * n * n + action_size(n) + 1) * 4;
		assert_eq!(bytes.len(), expected);
	}

	#[test]
	fn encode_sample_roundtrip_value() {
		let n = 5usize;
		let state_planes = vec![0.0f32; IN_CHANNELS * n * n];
		let policy = vec![0.0f32; action_size(n)];
		let bytes = encode_sample(&state_planes, &policy, -1.0);
		let last_f32 = f32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());
		assert_eq!(last_f32, -1.0);
	}
}
