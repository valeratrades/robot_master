use std::str::FromStr;

use rand::{rngs::SmallRng, seq::IteratorRandom};
use robot_master_core::game::{GameState, Move};
use v_utils::macros::CompactFormatNamed;

use crate::player::Bot;

#[derive(Clone, CompactFormatNamed, Debug, Default, Eq, PartialEq)]
pub struct Random {}

#[derive(Clone, Debug)]
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
impl std::fmt::Display for RandomPlayer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.params)
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
	[(); N + 1]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		game.valid_moves().choose(&mut self.rng).expect("no valid moves")
	}
}
