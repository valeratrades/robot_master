pub mod greedy;
pub mod random;
pub mod sadist;

use robot_master_arena::player::Player;

/// Resolve a player name to a concrete Player.
///
/// Returns `None` for "manual"/"m" — the caller handles manual input directly.
///
/// # Panics
/// Unknown player name.
pub fn resolve<const N: usize>(name: &str) -> Option<Box<dyn Player<N>>>
where
	[(); N * N]:, {
	match name {
		"m" | "manual" => None,
		"r" | "random" => Some(Box::new(random::RandomPlayer::new())),
		"g" | "greedy" => Some(Box::new(greedy::GreedyPlayer)),
		"s" | "sadist" => Some(Box::new(sadist::SadistPlayer)),
		other => panic!("unknown player algorithm: {other:?}. available: manual, random, greedy, sadist"),
	}
}
