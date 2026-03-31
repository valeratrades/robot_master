use std::str::FromStr;

use rand::{rngs::SmallRng, seq::IteratorRandom};
use robot_master_core::game::{GameState, Move};
use v_utils::macros::CompactFormatNamed;

use crate::player::Bot;

#[derive(Clone, CompactFormatNamed, Debug, Default, Eq, PartialEq)]
pub struct Random {}

#[derive(Clone, Debug, derive_more::Display)]
#[display("{params}")]
pub struct RandomPlayer {
	params: Random,
	rng: SmallRng,
}

impl Default for RandomPlayer {
	fn default() -> Self {
		Self {
			params: Random {},
			rng: rand::make_rng(),
		}
	}
}

impl PartialEq for RandomPlayer {
	fn eq(&self, other: &Self) -> bool {
		self.params == other.params
	}
}
impl Eq for RandomPlayer {}

impl FromStr for RandomPlayer {
	type Err = <Random as FromStr>::Err;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		s.parse::<Random>().map(|params| Self { params, rng: rand::make_rng() })
	}
}

impl<const N: usize> Bot<N> for RandomPlayer
where
	[(); N * N]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		game.valid_moves().choose(&mut self.rng).expect("no valid moves")
	}
}
