use rand::{Rng, seq::IteratorRandom};
use robot_master_core::game::{GameState, Move};

pub fn random_move<const N: usize>(state: &GameState<N>, rng: &mut impl Rng) -> Option<Move>
where
	[(); N * N]:, {
	state.valid_moves().choose(rng)
}
