pub mod greedy;
pub mod random;
pub mod sadist;

use std::{fmt, str::FromStr};

use ustr::{Ustr, ustr};

use crate::player::{Bot, ManualPlayer};

/// All non-manual algorithm names, in display order.
pub const ALGO_NAMES: &[&str] = &["random", "greedy", "sadist"];
/// Known player types.
#[derive(Clone, Debug)]
pub enum PlayerKind {
	Manual { name: String },
	Random,
	Greedy,
	Sadist,
}
impl PlayerKind {
	pub fn is_manual(&self) -> bool {
		matches!(self, PlayerKind::Manual { .. })
	}

	pub fn id(&self) -> Ustr {
		match self {
			PlayerKind::Manual { name } => ustr(name),
			PlayerKind::Random => ustr("random"),
			PlayerKind::Greedy => ustr("greedy"),
			PlayerKind::Sadist => ustr("sadist"),
		}
	}

	/// Construct a concrete `Bot<N>` from this kind.
	pub fn into_bot<const N: usize>(self) -> Box<dyn Bot<N>>
	where
		[(); N * N]:, {
		match self {
			PlayerKind::Manual { name } => Box::new(ManualPlayer::new(&name)),
			PlayerKind::Random => Box::new(random::RandomPlayer::new()),
			PlayerKind::Greedy => Box::new(greedy::GreedyPlayer),
			PlayerKind::Sadist => Box::new(sadist::SadistPlayer),
		}
	}
}

impl fmt::Display for PlayerKind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			PlayerKind::Manual { name } => f.write_str(name),
			PlayerKind::Random => f.write_str("Random"),
			PlayerKind::Greedy => f.write_str("Greedy"),
			PlayerKind::Sadist => f.write_str("Sadist"),
		}
	}
}

impl FromStr for PlayerKind {
	type Err = String;

	/// Case-insensitive matching with single-letter shortcuts.
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"m" | "manual" => Ok(PlayerKind::Manual { name: "Player".into() }),
			"r" | "random" => Ok(PlayerKind::Random),
			"g" | "greedy" => Ok(PlayerKind::Greedy),
			"s" | "sadist" => Ok(PlayerKind::Sadist),
			_ => Err(s.to_string()),
		}
	}
}
