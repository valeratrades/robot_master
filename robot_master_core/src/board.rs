use crate::game::PlayerId;

pub const EMPTY: Cell = u8::MAX;
pub type Cell = u8;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Copy, Debug)]
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
	pub fn line(&self, player: PlayerId, idx: usize) -> [Cell; N] {
		let mut out = [EMPTY; N];
		for j in 0..N {
			out[j] = if player.scores_rows() { self.cells[idx * N + j] } else { self.cells[j * N + idx] };
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
}

impl<const N: usize> Default for Board<N>
where
	[(); N * N]:,
{
	fn default() -> Self {
		// Can't use [EMPTY; N*N] via derive (Default for arrays requires T: Default, but EMPTY=u8::MAX ≠ 0).
		Self { cells: [u8::MAX; N * N] }
	}
}

pub type Board5 = Board<5>;
pub type Board7 = Board<7>;
pub type Board9 = Board<9>;
pub type Board11 = Board<11>;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::game::PlayerId;

	fn center5() -> Board5 {
		let mut b = Board5::default();
		b.set(Pos { row: 2, col: 2 }, 3);
		b
	}

	#[test]
	fn new_board_all_empty() {
		let b = Board5::default();
		for row in 0..5u8 {
			for col in 0..5u8 {
				assert!(b.is_empty(Pos { row, col }));
			}
		}
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
		let mut b = Board5::default();
		b.set(Pos { row: 0, col: 0 }, 1);
		b.set(Pos { row: 1, col: 0 }, 2);
		b.set(Pos { row: 2, col: 0 }, 3);

		let col = b.line(PlayerId::Cols, 0);
		assert_eq!(col[0], 1);
		assert_eq!(col[1], 2);
		assert_eq!(col[2], 3);
		assert_eq!(col[3], EMPTY);

		let row = b.line(PlayerId::Rows, 1);
		assert_eq!(row[0], 2);
		assert_eq!(row[1], EMPTY);
	}
}
