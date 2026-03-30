use rand::{rngs::SmallRng, seq::IteratorRandom};
use robot_master_core::game::{GameState, Move};
use ustr::{Ustr, ustr};

use crate::player::Player;

pub struct RandomPlayer {
	rng: SmallRng,
}
impl RandomPlayer {
	pub fn new() -> Self {
		Self::default()
	}
}

impl Default for RandomPlayer {
	fn default() -> Self {
		Self { rng: rand::make_rng() }
	}
}

impl<const N: usize> Player<N> for RandomPlayer
where
	[(); N * N]:,
{
	fn id(&self) -> Ustr {
		ustr("random")
	}

	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		game.valid_moves().choose(&mut self.rng).expect("no valid moves")
	}
}
