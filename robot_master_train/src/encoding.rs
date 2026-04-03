use robot_master_core::{
	board::EMPTY,
	game::{GameState, Player},
};

/// Number of input channels for an N×N board.
///
/// Layout:
///   [0:N+1]         card value presence — plane `v` for values 0..=N
///   [N+1]           empty cells
///   [N+2]           playable cells
///   [N+3:3N+5]      current player hand — 2 planes per value: ≥1 copy, ≥2 copies
///   [3N+5:5N+7]     opponent hand       — same encoding
///   [5N+7]          turn indicator
///   Total: 5N + 8
pub const fn in_channels(n: usize) -> usize {
	5 * n + 8
}

/// Encode `GameState<N>` into `in_channels(N) * N * N` f32 values, channel-first (CHW).
pub fn encode_planes<const N: usize>(state: &GameState<N>) -> Vec<f32>
where
	[(); N * N]:,
	[(); N + 1]:, {
	let n2 = N * N;
	let channels = in_channels(N);
	let mut planes = vec![0.0f32; channels * n2];

	let set = |planes: &mut Vec<f32>, ch: usize, row: usize, col: usize, v: f32| {
		planes[ch * n2 + row * N + col] = v;
	};

	// Channel offsets derived from N:
	let ch_empty = N + 1;
	let ch_playable = N + 2;
	let ch_hand_cur = N + 3; // 2 planes per value: [ch + v*2], [ch + v*2 + 1]
	let ch_hand_opp = 3 * N + 5; // same layout
	let ch_turn = 5 * N + 7;

	for row in 0..N {
		for col in 0..N {
			let cell = state.board.get(robot_master_core::board::Pos { row: row as u8, col: col as u8 });
			if cell == EMPTY {
				set(&mut planes, ch_empty, row, col, 1.0);
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
				set(&mut planes, ch_playable, row, col, 1.0);
			}
		}
	}

	let current_idx = state.turn.index() as usize;
	let opponent_idx = 1 - current_idx;
	let hands = state.hands().expect("encoding requires visible hands");
	let hand_cur = &hands[current_idx];
	let hand_opp = &hands[opponent_idx];

	// broadcast: fill entire plane with 1.0
	let fill_plane = |planes: &mut Vec<f32>, ch: usize| {
		let start = ch * n2;
		planes[start..start + n2].fill(1.0);
	};

	for v in 0..=N {
		let card = robot_master_core::cards::CardValue(v as u8);
		let cnt_cur = hand_cur.count(card);
		let cnt_opp = hand_opp.count(card);

		if cnt_cur >= 1 {
			fill_plane(&mut planes, ch_hand_cur + v * 2);
		}
		if cnt_cur >= 2 {
			fill_plane(&mut planes, ch_hand_cur + v * 2 + 1);
		}
		if cnt_opp >= 1 {
			fill_plane(&mut planes, ch_hand_opp + v * 2);
		}
		if cnt_opp >= 2 {
			fill_plane(&mut planes, ch_hand_opp + v * 2 + 1);
		}
	}

	if state.turn == Player::A {
		let start = ch_turn * n2;
		planes[start..start + n2].fill(1.0);
	}

	planes
}

/// Number of possible actions: (N+1) card values × N² positions.
pub const fn action_size(n: usize) -> usize {
	(n + 1) * n * n
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
	use robot_master_core::game::{GameConfig, GameState, Player, PlayerSigned};

	use super::*;

	fn state5() -> GameState<5> {
		let mut rng = SmallRng::seed_from_u64(42);
		GameState::new(GameConfig::default(), &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)])
	}

	#[test]
	fn planes_shape() {
		let state = state5();
		let planes = encode_planes(&state);
		assert_eq!(planes.len(), in_channels(5) * 5 * 5);
	}

	#[test]
	fn planes_center_card() {
		let state = state5();
		// Center is (2,2). Whatever card is there should have its plane set to 1.0.
		let center_cell = state.board.get(robot_master_core::board::Pos { row: 2, col: 2 });
		assert_ne!(center_cell, EMPTY);
		let planes = encode_planes(&state);
		assert_eq!(planes[center_cell as usize * 25 + 2 * 5 + 2], 1.0, "card-value plane at center");
		// empty channel for N=5 is N+1=6
		assert_eq!(planes[6 * 25 + 2 * 5 + 2], 0.0, "empty plane not set at center");
	}

	#[test]
	fn planes_playable_adjacent_to_center() {
		let state = state5();
		let planes = encode_planes(&state);
		// playable channel for N=5 is N+2=7
		// (2,1) is adjacent to center (2,2) and should be playable at game start
		assert_eq!(planes[7 * 25 + 2 * 5 + 1], 1.0, "adjacent to center is playable");
		// (0,0) is not adjacent to anything occupied
		assert_eq!(planes[7 * 25 + 0 * 5 + 0], 0.0, "corner is not playable");
	}

	#[test]
	fn turn_indicator_player_a() {
		let state = state5(); // starts as Player::A
		let planes = encode_planes(&state);
		// turn channel for N=5 is 5*5+7=32
		assert_eq!(planes[32 * 25], 1.0, "player A indicator");
	}

	#[test]
	fn encode_sample_byte_length() {
		let n = 5usize;
		let state_planes = vec![0.0f32; in_channels(n) * n * n];
		let policy = vec![0.0f32; action_size(n)];
		let bytes = encode_sample(&state_planes, &policy, 1.0);
		let expected = (in_channels(n) * n * n + action_size(n) + 1) * 4;
		assert_eq!(bytes.len(), expected);
	}

	#[test]
	fn encode_sample_roundtrip_value() {
		let n = 5usize;
		let state_planes = vec![0.0f32; in_channels(n) * n * n];
		let policy = vec![0.0f32; action_size(n)];
		let bytes = encode_sample(&state_planes, &policy, -1.0);
		let last_f32 = f32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());
		assert_eq!(last_f32, -1.0);
	}
}
