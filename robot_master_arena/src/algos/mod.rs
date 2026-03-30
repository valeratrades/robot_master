pub mod greedy;
pub mod random;
pub mod rollout;
pub mod sadist;

use std::{fmt, str::FromStr};

use ustr::{Ustr, ustr};
use v_utils::macros::CompactFormatNamed;

use crate::player::{Bot, ManualPlayer};

/// All non-manual algorithm names, in display order.
pub const ALGO_NAMES: &[&str] = &["random", "greedy", "sadist", "rollout", "mcts"];

/// MCTS parameters, parsed via CompactFormatNamed.
/// CLI: `mcts:s200` (simulations=200). Bare `mcts` uses defaults.
#[derive(Clone, Debug, CompactFormatNamed)]
pub struct MctsParams {
	pub simulations: u32,
}

impl Default for MctsParams {
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
	Random,
	Greedy,
	Sadist,
	Rollout,
	/// MCTS with rollout evaluation. `into_bot` cannot construct this — the binary crate
	/// must handle it via `robot_master_train::mcts`.
	Mcts(MctsParams),
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
			PlayerKind::Rollout => ustr("rollout"),
			PlayerKind::Mcts(_) => ustr("mcts"),
		}
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
			PlayerKind::Random => Box::new(random::RandomPlayer::new()),
			PlayerKind::Greedy => Box::new(greedy::GreedyPlayer),
			PlayerKind::Sadist => Box::new(sadist::SadistPlayer),
			PlayerKind::Rollout => Box::new(rollout::RolloutPlayer),
			PlayerKind::Mcts(_) => panic!("Mcts cannot be constructed via into_bot; use robot_master_train::mcts::MctsBot"),
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
			PlayerKind::Rollout => f.write_str("Rollout"),
			PlayerKind::Mcts(params) => write!(f, "MCTS({})", params.simulations),
		}
	}
}

impl FromStr for PlayerKind {
	type Err = String;

	/// Case-insensitive matching with single-letter shortcuts.
	/// Parameterized variants accept compact format: `mcts:s200`.
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let lower = s.to_lowercase();
		let name = lower.split(':').next().unwrap();

		match name {
			"m" | "manual" => Ok(PlayerKind::Manual { name: "Player".into() }),
			"r" | "random" => Ok(PlayerKind::Random),
			"g" | "greedy" => Ok(PlayerKind::Greedy),
			"s" | "sadist" => Ok(PlayerKind::Sadist),
			"ro" | "rollout" => Ok(PlayerKind::Rollout),
			"mcts" =>
				if s.contains(':') {
					let params: MctsParams = s.parse().map_err(|e: v_utils::__internal::eyre::Report| e.to_string())?;
					Ok(PlayerKind::Mcts(params))
				} else {
					Ok(PlayerKind::Mcts(MctsParams::default()))
				},
			_ => Err(s.to_string()),
		}
	}
}
