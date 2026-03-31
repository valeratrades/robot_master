pub mod greedy;
pub mod random;
pub mod rollout;
pub mod sadist;

use std::{fmt, str::FromStr};

pub use greedy::Greedy;
pub use random::Random;
pub use rollout::Rollout;
pub use sadist::Sadist;
use ustr::{Ustr, ustr};
use v_utils::macros::CompactFormatNamed;

use crate::player::{Bot, ManualPlayer};

/// MCTS parameters.
/// CLI: `mcts:s200` (simulations=200). Bare `mcts` uses defaults.
#[derive(Clone, CompactFormatNamed, Debug)]
pub struct Mcts {
	pub simulations: u32,
}

impl Default for Mcts {
	fn default() -> Self {
		Self { simulations: 200 }
	}
}

/// Known player types.
#[derive(Clone, Debug)]
pub enum PlayerKind {
	Manual {
		name: String,
	},
	Random(Random),
	Greedy(Greedy),
	Sadist(Sadist),
	Rollout(Rollout),
	/// MCTS with rollout evaluation. `into_bot` cannot construct this — the binary crate
	/// must handle it via `robot_master_train::mcts`.
	Mcts(Mcts),
}

impl PlayerKind {
	pub fn is_manual(&self) -> bool {
		matches!(self, PlayerKind::Manual { .. })
	}

	pub fn id(&self) -> Ustr {
		ustr(&self.to_string())
	}

	/// Canonical algo names for discovery (fzf, arena seeding).
	pub fn algo_names() -> &'static [&'static str] {
		&["random", "greedy", "sadist", "rollout", "mcts"]
	}

	/// Construct a concrete `Bot<N>` from this kind.
	///
	/// # Panics
	/// Panics for `Mcts` — use `robot_master_train::mcts::MctsBot` directly.
	pub fn into_bot<const N: usize>(self) -> Box<dyn Bot<N>>
	where
		[(); N * N]:, {
		match self {
			PlayerKind::Manual { name } => Box::new(ManualPlayer::new(&name)),
			PlayerKind::Random(_) => Box::new(random::RandomPlayer::new()),
			PlayerKind::Greedy(_) => Box::new(greedy::Greedy {}),
			PlayerKind::Sadist(_) => Box::new(sadist::Sadist {}),
			PlayerKind::Rollout(_) => Box::new(rollout::Rollout {}),
			PlayerKind::Mcts(_) => panic!("Mcts cannot be constructed via into_bot; use robot_master_train::mcts::MctsBot"),
		}
	}
}

impl fmt::Display for PlayerKind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			PlayerKind::Manual { name } => write!(f, "manual:{name}"),
			PlayerKind::Random(p) => write!(f, "{p}"),
			PlayerKind::Greedy(p) => write!(f, "{p}"),
			PlayerKind::Sadist(p) => write!(f, "{p}"),
			PlayerKind::Rollout(p) => write!(f, "{p}"),
			PlayerKind::Mcts(p) => write!(f, "{p}"),
		}
	}
}

impl FromStr for PlayerKind {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		// Try each CompactFormatNamed struct. Order doesn't matter — graphemics ensure
		// only the matching struct name succeeds.
		if let Ok(v) = s.parse::<Mcts>() {
			return Ok(PlayerKind::Mcts(v));
		}
		if let Ok(v) = s.parse::<Random>() {
			return Ok(PlayerKind::Random(v));
		}
		if let Ok(v) = s.parse::<Greedy>() {
			return Ok(PlayerKind::Greedy(v));
		}
		if let Ok(v) = s.parse::<Sadist>() {
			return Ok(PlayerKind::Sadist(v));
		}
		if let Ok(v) = s.parse::<Rollout>() {
			return Ok(PlayerKind::Rollout(v));
		}

		// Bare "mcts" without params.
		if s.eq_ignore_ascii_case("mcts") {
			return Ok(PlayerKind::Mcts(Mcts::default()));
		}

		// Manual players: "manual:<name>" or bare "manual".
		let lower = s.to_lowercase();
		if let Some(name) = lower.strip_prefix("manual:") {
			validate_manual_name(name)?;
			return Ok(PlayerKind::Manual { name: name.to_string() });
		}
		if lower == "manual" {
			return Ok(PlayerKind::Manual { name: "Player".into() });
		}

		Err(s.to_string())
	}
}

/// Manual player names must be alphanumeric (plus `_` and `-`).
pub fn validate_manual_name(name: &str) -> Result<(), String> {
	if name.is_empty() {
		return Err("manual player name cannot be empty".into());
	}
	if let Some(c) = name.chars().find(|c| !c.is_ascii_alphanumeric() && *c != '_' && *c != '-') {
		return Err(format!("invalid character '{c}' in manual player name \"{name}\" (allowed: a-zA-Z0-9_-)"));
	}
	Ok(())
}
