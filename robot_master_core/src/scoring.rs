use crate::{
	board::{Board, Cell, EMPTY},
	cards::{CardValue, MAX_SUPPORTED_CARD_VALUE},
	game::Player,
};

pub type LineCounts = [u8; MAX_SUPPORTED_CARD_VALUE + 1];

pub fn line_counts(line: &[Cell]) -> LineCounts {
	let mut c = [0u8; MAX_SUPPORTED_CARD_VALUE + 1];
	for &cell in line {
		if cell != EMPTY {
			c[cell as usize] += 1;
		}
	}
	c
}

/// 1 copy → face value; 2 copies → 10×face; 3+ → 100 flat.
pub fn score_line(counts: &LineCounts) -> u16 {
	let mut s = 0u16;
	for (v, &c) in counts.iter().enumerate() {
		s += match c {
			0 => 0,
			1 => v as u16,
			2 => 10 * v as u16,
			_ => 100,
		};
	}
	s
}

/// Analytic score delta when card v is added to a line with the given counts.
/// 0→1: +v; 1→2: +9v; 2→3: +(100−10v); 3+: 0
pub fn score_delta(counts: &LineCounts, v: CardValue) -> i16 {
	let vv = v.0 as i16;
	match counts[v.0 as usize] {
		0 => vv,
		1 => 9 * vv,
		2 => 100 - 10 * vv,
		_ => 0,
	}
}

/// Final result: (score_p0, idx_p0, score_p1, idx_p1)
pub fn victoire<const N: usize>(board: &Board<N>) -> (u16, usize, u16, usize)
where
	[(); N * N]:, {
	let (s0, i0) = player_score(board, Player::A);
	let (s1, i1) = player_score(board, Player::B);
	(s0, i0, s1, i1)
}
/// Minimum score across all of a player's lines. Returns (min_score, line_index).
fn player_score<const N: usize>(board: &Board<N>, player: Player) -> (u16, usize)
where
	[(); N * N]:, {
	(0..N)
		.map(|i| (score_line(&line_counts(&board.line(player, i))), i))
		.min_by_key(|&(s, _)| s)
		.expect("board has no lines")
}

#[cfg(test)]
mod tests {
	use super::*;

	fn counts(pairs: &[(usize, u8)]) -> LineCounts {
		let mut c = [0u8; MAX_SUPPORTED_CARD_VALUE + 1];
		for &(v, n) in pairs {
			c[v] = n;
		}
		c
	}

	// score_line

	#[test]
	fn score_zero_copies_contributes_nothing() {
		let c = counts(&[]);
		assert_eq!(score_line(&c), 0);
	}

	#[test]
	fn score_one_copy_is_face_value() {
		for v in 0u16..=5 {
			let c = counts(&[(v as usize, 1)]);
			assert_eq!(score_line(&c), v);
		}
	}

	#[test]
	fn score_two_copies_is_ten_times_face() {
		for v in 0u16..=5 {
			let c = counts(&[(v as usize, 2)]);
			assert_eq!(score_line(&c), 10 * v);
		}
	}

	#[test]
	fn score_three_plus_is_100_flat() {
		for n in 3u8..=6 {
			let c = counts(&[(3, n)]);
			assert_eq!(score_line(&c), 100);
		}
	}

	#[test]
	fn score_line_mixed() {
		// 1 copy of 2 (=2) + 2 copies of 5 (=50) = 52
		let c = counts(&[(2, 1), (5, 2)]);
		assert_eq!(score_line(&c), 52);
	}

	// score_delta

	#[test]
	fn delta_first_copy() {
		let c = counts(&[]);
		assert_eq!(score_delta(&c, CardValue(3)), 3);
		assert_eq!(score_delta(&c, CardValue(0)), 0);
	}

	#[test]
	fn delta_second_copy() {
		let c = counts(&[(3, 1)]);
		assert_eq!(score_delta(&c, CardValue(3)), 27); // 9 * 3
	}

	#[test]
	fn delta_third_copy() {
		let c = counts(&[(2, 2), (5, 2)]);
		assert_eq!(score_delta(&c, CardValue(2)), 80); // 100 - 10*2
		assert_eq!(score_delta(&c, CardValue(5)), 50); // 100 - 10*5
	}

	#[test]
	fn delta_saturation() {
		let c = counts(&[(1, 3), (2, 4), (3, 5)]);
		assert_eq!(score_delta(&c, CardValue(1)), 0);
		assert_eq!(score_delta(&c, CardValue(2)), 0);
		assert_eq!(score_delta(&c, CardValue(3)), 0);
	}

	// line_counts skips EMPTY

	#[test]
	fn line_counts_skips_empty() {
		let line = [EMPTY, 2, EMPTY, 2, 3];
		let c = line_counts(&line);
		assert_eq!(c[2], 2);
		assert_eq!(c[3], 1);
		assert_eq!(c[0], 0);
	}
}
