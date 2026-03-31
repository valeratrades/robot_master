use rand::{rngs::SmallRng, seq::IteratorRandom};
use robot_master_core::game::{GameState, Move};
use ustr::{Ustr, ustr};
use v_utils::macros::CompactFormatNamed;

use crate::player::Bot;

/// Separate from `RandomPlayer` because `SmallRng` doesn't impl `FromStr`/`Display`,
/// so it can't be a field on a `CompactFormatNamed` struct.
#[derive(Clone, CompactFormatNamed, Debug)]
pub struct Random {}

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

impl<const N: usize> Bot<N> for RandomPlayer
where
	[(); N * N]:,
{
	fn id(&self) -> Ustr {
		ustr(&Random {}.to_string())
	}

	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		game.valid_moves().choose(&mut self.rng).expect("no valid moves")
	}
}
