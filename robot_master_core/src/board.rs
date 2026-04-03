use std::fmt;

use crate::game::{Player, scores_rows};

pub const EMPTY: Cell = u8::MAX;
pub type Cell = u8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Pos {
	pub row: u8,
	pub col: u8,
}

impl Pos {
	#[inline]
	pub fn flat<const N: usize>(self) -> usize {
		self.row as usize * N + self.col as usize
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Board<const N: usize>
where
	[(); N * N]:, {
	cells: [Cell; N * N],
}
impl<const N: usize> Board<N>
where
	[(); N * N]:,
{
	#[inline]
	pub fn get(&self, p: Pos) -> Cell {
		self.cells[p.flat::<N>()]
	}

	#[inline]
	pub fn set(&mut self, p: Pos, v: Cell) {
		self.cells[p.flat::<N>()] = v;
	}

	#[inline]
	pub fn is_empty(&self, p: Pos) -> bool {
		self.get(p) == EMPTY
	}

	pub fn is_full(&self) -> bool {
		self.cells.iter().all(|&c| c != EMPTY)
	}

	/// True if pos is in-bounds, currently empty, and has at least one occupied neighbour.
	pub fn is_playable(&self, p: Pos) -> bool {
		if p.row as usize >= N || p.col as usize >= N {
			return false;
		}
		if !self.is_empty(p) {
			return false;
		}
		for (p2, _) in self.neighbours(p) {
			if !self.is_empty(p2) {
				return true;
			}
		}
		false
	}

	pub fn valid_placements(&self) -> impl Iterator<Item = Pos> + '_ {
		(0..N).flat_map(move |row| {
			(0..N).filter_map(move |col| {
				let p = Pos { row: row as u8, col: col as u8 };
				self.is_playable(p).then_some(p)
			})
		})
	}

	/// Row (Rows player) or column (Cols player) as a fixed array.
	pub fn line(&self, player: Player, idx: usize) -> [Cell; N] {
		let mut out = [EMPTY; N];
		for (j, slot) in out.iter_mut().enumerate() {
			*slot = if scores_rows(player) { self.cells[idx * N + j] } else { self.cells[j * N + idx] };
		}
		out
	}

	fn neighbours(&self, p: Pos) -> impl Iterator<Item = (Pos, Cell)> + '_ {
		let row = p.row as isize;
		let col = p.col as isize;
		[(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().filter_map(move |(dr, dc)| {
			let r = row + dr;
			let c = col + dc;
			if r >= 0 && r < N as isize && c >= 0 && c < N as isize {
				let p2 = Pos { row: r as u8, col: c as u8 };
				Some((p2, self.get(p2)))
			} else {
				None
			}
		})
	}

	/// Diff display against another board state.
	/// - `+v` — cell added (was empty in `other`, filled here)
	/// - `-v` — cell removed (filled in `other`, empty here)
	/// - `~v` — cell changed value
	/// - ` v` — unchanged
	pub fn display_diff(&self, other: &Board<N>) -> String {
		use fmt::Write;
		let mut out = String::new();
		let bar: String = "-".repeat(9 + 4 * N);
		writeln!(out, "{bar}").unwrap();
		write!(out, "          ").unwrap();
		for c in 0..N {
			if c + 1 < N {
				write!(out, "{c}   ").unwrap();
			} else {
				write!(out, "{c}").unwrap();
			}
		}
		writeln!(out).unwrap();
		writeln!(out, "{bar}").unwrap();
		for row in 0..N {
			write!(out, "({row},_)   |").unwrap();
			for col in 0..N {
				let p = Pos { row: row as u8, col: col as u8 };
				let mine = self.get(p);
				let theirs = other.get(p);
				match (mine, theirs) {
					(a, b) if a == b =>
						if a == EMPTY {
							write!(out, "   |").unwrap();
						} else {
							write!(out, " {a} |").unwrap();
						},
					(a, b) if b == EMPTY => write!(out, "+{a} |").unwrap(),
					(a, _) if a == EMPTY => write!(out, "-  |").unwrap(),
					(a, _) => write!(out, "~{a} |").unwrap(),
				}
			}
			writeln!(out).unwrap();
		}
		write!(out, "{bar}").unwrap();
		out
	}
}

impl<const N: usize> Default for Board<N>
where
	[(); N * N]:,
{
	fn default() -> Self {
		Self { cells: [u8::MAX; N * N] }
	}
}

impl<const N: usize> fmt::Display for Board<N>
where
	[(); N * N]:,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let bar: String = "-".repeat(9 + 4 * N);
		writeln!(f, "{bar}")?;
		write!(f, "          ")?;
		for c in 0..N {
			if c + 1 < N {
				write!(f, "{c}   ")?;
			} else {
				write!(f, "{c}")?;
			}
		}
		writeln!(f)?;
		writeln!(f, "{bar}")?;
		for row in 0..N {
			write!(f, "({row},_)   |")?;
			for col in 0..N {
				let cell = self.get(Pos { row: row as u8, col: col as u8 });
				if cell == EMPTY {
					write!(f, "   |")?;
				} else {
					write!(f, " {cell} |")?;
				}
			}
			writeln!(f)?;
		}
		write!(f, "{bar}")?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::game::Player;

	fn center5() -> Board<5> {
		let mut b = Board::<5>::default();
		b.set(Pos { row: 2, col: 2 }, 3);
		b
	}

	#[test]
	fn is_full_false_then_true() {
		let mut b = Board::<1>::default();
		assert!(!b.is_full());
		b.set(Pos { row: 0, col: 0 }, 0);
		assert!(b.is_full());
	}

	#[test]
	fn is_playable_requires_neighbour() {
		let b = center5();
		assert!(!b.is_playable(Pos { row: 2, col: 2 }));
		assert!(b.is_playable(Pos { row: 2, col: 1 }));
		assert!(b.is_playable(Pos { row: 1, col: 2 }));
		assert!(!b.is_playable(Pos { row: 0, col: 0 }));
	}

	#[test]
	fn is_playable_out_of_bounds() {
		let b = center5();
		assert!(!b.is_playable(Pos { row: 5, col: 2 }));
		assert!(!b.is_playable(Pos { row: 2, col: 5 }));
	}

	#[test]
	fn valid_placements_center_only() {
		let b = center5();
		let placements: Vec<_> = b.valid_placements().collect();
		assert_eq!(placements.len(), 4);
	}

	#[test]
	fn line_row_col() {
		let mut b = Board::<5>::default();
		b.set(Pos { row: 0, col: 0 }, 1);
		b.set(Pos { row: 1, col: 0 }, 2);
		b.set(Pos { row: 2, col: 0 }, 3);

		let col = b.line(Player::A, 0);
		assert_eq!(col[0], 1);
		assert_eq!(col[1], 2);
		assert_eq!(col[2], 3);
		assert_eq!(col[3], EMPTY);

		let row = b.line(Player::B, 1);
		assert_eq!(row[0], 2);
		assert_eq!(row[1], EMPTY);
	}
}
