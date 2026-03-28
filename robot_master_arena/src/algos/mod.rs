pub mod greedy;
pub mod random;
pub mod sadist;

use crate::player::{ManualPlayer, Player};

/// Resolve a player name to a concrete Player.
///
/// "manual"/"m" returns a `ManualPlayer` — the caller must supply moves via `Match::next(Some(m))`.
///
/// # Panics
/// Unknown player name.
pub fn resolve<const N: usize>(name: &str) -> Box<dyn Player<N>>
where
	[(); N * N]:, {
	match name {
		"m" | "manual" => Box::new(ManualPlayer::new(name)),
		"r" | "random" => Box::new(random::RandomPlayer::new()),
		"g" | "greedy" => Box::new(greedy::GreedyPlayer),
		"s" | "sadist" => Box::new(sadist::SadistPlayer),
		other => panic!("unknown player algorithm: {other:?}. available: manual, random, greedy, sadist"),
	}
}

/// Returns true if the player name refers to a manual/human player.
pub fn is_manual(name: &str) -> bool {
	matches!(name, "m" | "manual")
}
