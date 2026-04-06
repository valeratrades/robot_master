use robot_master_core::{
	board::EMPTY,
	game::{GameState, Player},
};

/// Number of input channels for an N×N board.
///
/// Layout:
///   [0:N+1]       card value presence — plane `v` is 1.0 where card value v is placed
///   [N+1:2N+2]    current player hand — one plane per value, count/(N+1) normalized
///   [2N+2:3N+3]   opponent hand       — same encoding
///   Total: 3N + 3
///
/// Removed vs previous versions:
///   - empty/playable planes: derivable by the conv net from card planes
///   - turn indicator: board is transposed for Player B, encoding is player-invariant
///   - 2-plane-per-value hand encoding: collapsed to single normalized float plane
pub const fn in_channels(n: usize) -> usize {
	3 * n + 3
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

	let ch_hand_cur = N + 1; // one plane per value: count / (N+1)
	let ch_hand_opp = 2 * N + 2;

	// When it's Player B's turn, transpose board reads (row↔col) so that both
	// players always see "my scoring dimension is columns". The network learns
	// one strategy instead of two.
	let bpos = |row: usize, col: usize| -> robot_master_core::board::Pos {
		if state.turn == Player::B {
			robot_master_core::board::Pos { row: col as u8, col: row as u8 }
		} else {
			robot_master_core::board::Pos { row: row as u8, col: col as u8 }
		}
	};

	for row in 0..N {
		for col in 0..N {
			let cell = state.board.get(bpos(row, col));
			if cell != EMPTY {
				set(&mut planes, cell as usize, row, col, 1.0);
			}
		}
	}

	let current_idx = state.turn.index() as usize;
	let opponent_idx = 1 - current_idx;
	let hands = state.hands().expect("encoding requires visible hands");
	let hand_cur = &hands[current_idx];
	let hand_opp = &hands[opponent_idx];

	// Hand planes: normalized count so the network sees a meaningful magnitude.
	// Max copies of any single card value is N+1 (one of each suit), so divide by N+1.
	let norm = (N + 1) as f32;
	let fill_plane = |planes: &mut Vec<f32>, ch: usize, val: f32| {
		let start = ch * n2;
		planes[start..start + n2].fill(val);
	};

	for v in 0..=N {
		let card = robot_master_core::cards::CardValue(v as u8);
		let cnt_cur = hand_cur.count(card) as f32 / norm;
		let cnt_opp = hand_opp.count(card) as f32 / norm;
		if cnt_cur > 0.0 {
			fill_plane(&mut planes, ch_hand_cur + v, cnt_cur);
		}
		if cnt_opp > 0.0 {
			fill_plane(&mut planes, ch_hand_opp + v, cnt_opp);
		}
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
/// Pass `player_b = true` when encoding from Player B's perspective (board is
/// transposed in `encode_planes`), so row↔col are swapped to stay consistent.
pub fn action_index(card: u8, row: usize, col: usize, n: usize, player_b: bool) -> usize {
	let (r, c) = if player_b { (col, row) } else { (row, col) };
	card as usize * n * n + r * n + c
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
	}

	#[test]
	fn hand_plane_normalized() {
		let state = state5();
		let planes = encode_planes(&state);
		// ch_hand_cur starts at N+1 = 6. Each plane should be in [0, 1].
		let ch_hand_cur = 5 + 1;
		for v in 0..=5usize {
			let val = planes[(ch_hand_cur + v) * 25];
			assert!(val >= 0.0 && val <= 1.0, "hand plane {v} out of range: {val}");
		}
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
