pub mod greedy_max;
pub mod greedy_min;
pub mod random;
pub mod rollout;
pub mod sadist;
mod test_utils;

pub use greedy_max::GreedyForNumber;
pub use greedy_min::GreedyForScore;
pub use random::RandomPlayer;
pub use rollout::Rollout;
pub use sadist::Sadist;
use strum::IntoEnumIterator;
use ustr::{Ustr, ustr};
use v_utils::macros::CompactFormatNamed;

use crate::player::{Bot, ManualPlayer};

/// MCTS parameters.
/// CLI: `mcts:s200` (simulations=200). Bare `mcts` uses defaults.
#[derive(Clone, CompactFormatNamed, Debug, Default, Eq, PartialEq)]
pub struct Mcts {
	pub simulations: u32 = 200,
}

/// Known player types.
//NB: pay attention to keeping field names equal to contained values. Easy to rename contained T via lsp from elsewhere, and forget to rename the field name to follow.
#[derive(Clone, Debug, derive_more::Display, strum::EnumIter, Eq, PartialEq, v_utils::macros::TryParseVariants)]
pub enum PlayerKind {
	ManualPlayer(ManualPlayer),
	RandomPlayer(RandomPlayer),
	GreedyForNumber(GreedyForNumber),
	GreedyForScocre(GreedyForScore),
	Sadist(Sadist),
	Rollout(Rollout),
	/// MCTS with rollout evaluation. `into_bot` cannot construct this — the binary crate
	/// must handle it via `robot_master_train::mcts`.
	Mcts(Mcts),
}

impl PlayerKind {
	pub fn is_manual(&self) -> bool {
		matches!(self, PlayerKind::ManualPlayer(_))
	}

	pub fn id(&self) -> Ustr {
		ustr(&self.to_string())
	}

	/// All non-Manual variants with default parameters.
	pub fn defaults() -> Vec<PlayerKind> {
		Self::iter().filter(|k| !k.is_manual()).collect()
	}

	/// Construct a concrete `Bot<N>` from this kind.
	///
	/// # Panics
	/// Panics for `Mcts` — use `robot_master_train::mcts::MctsBot` directly.
	pub fn into_bot<const N: usize>(self) -> Box<dyn Bot<N>>
	where
		[(); N * N]:, {
		match self {
			PlayerKind::ManualPlayer(p) => Box::new(p),
			PlayerKind::RandomPlayer(p) => Box::new(p),
			PlayerKind::GreedyForNumber(p) => Box::new(p),
			PlayerKind::GreedyForScocre(p) => Box::new(p),
			PlayerKind::Sadist(p) => Box::new(p),
			PlayerKind::Rollout(p) => Box::new(p),
			PlayerKind::Mcts(_) => panic!("Mcts cannot be constructed via into_bot; use robot_master_train::mcts::MctsBot"),
		} // have to hardcode names even though we do the same thing, cause eg `Mcts` is attached later (round dependencies), so at this level it doesn't impl `Bot`
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
