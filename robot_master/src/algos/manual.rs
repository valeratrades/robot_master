use robot_master_arena::player::Player;
use robot_master_core::game::{GameState, Move};
use ustr::{Ustr, ustr};

/// Placeholder player for manual/human input.
///
/// `choose_move` deliberately panics — interfaces must detect Manual players
/// and provide moves externally via `Match::next(Some(move))`.
pub struct Manual {
	id: Ustr,
}

impl Manual {
	pub fn new(name: &str) -> Self {
		Self { id: ustr(name) }
	}
}

impl<const N: usize> Player<N> for Manual
where
	[(); N * N]:,
{
	fn id(&self) -> Ustr {
		self.id
	}

	fn choose_move(&mut self, _game: &GameState<N>) -> Move {
		panic!("Manual::choose_move called — interfaces must handle Manual players explicitly and provide moves via Match::next(Some(m))")
	}
}
