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

use crate::player::{Bot, ManualPlayer};

/// Which search algorithm wraps the inner evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchKind {
	/// Vanilla UCT-MCTS: uniform exploration, picks most-visited child.
	Vanilla,
	/// Gumbel Sequential Halving: guided by learned priors + Gumbel noise.
	Gumbel,
}

/// An ONNX model file exposed as a player. Carries only the stem; path resolution
/// happens in the binary crate against the user-specified models dir.
///
/// Does not implement `Bot<N>` — the binary crate constructs the actual GumbelBot+NnEval.
#[derive(Clone, Debug, Default, derive_more::Display, Eq, PartialEq)]
#[display("onnx:{stem}")]
pub struct OnnxPlayer {
	pub stem: String,
}

impl std::str::FromStr for OnnxPlayer {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let stem = s.strip_prefix("onnx:").ok_or_else(|| s.to_string())?;
		if stem.is_empty() {
			return Err(s.to_string());
		}
		Ok(Self { stem: stem.to_string() })
	}
}

/// The underlying player algorithm, without Gumbel wrapping.
#[derive(Clone, Debug, derive_more::Display, strum::EnumIter, Eq, PartialEq, v_utils::macros::TryParseVariants)]
pub enum InnerKind {
	ManualPlayer(ManualPlayer),
	RandomPlayer(RandomPlayer),
	GreedyForNumber(GreedyForNumber),
	GreedyForScocre(GreedyForScore),
	Sadist(Sadist),
	Rollout(Rollout),
	/// ONNX model. `into_bot` cannot construct this — the binary crate must handle it.
	OnnxPlayer(OnnxPlayer),
}

impl InnerKind {
	pub fn is_manual(&self) -> bool {
		matches!(self, InnerKind::ManualPlayer(_))
	}

	pub fn is_onnx(&self) -> bool {
		matches!(self, InnerKind::OnnxPlayer(_))
	}

	/// Construct a direct (non-Gumbel) `Bot<N>`.
	///
	/// # Panics
	/// Panics for `OnnxPlayer` — the binary crate must construct `GumbelBot<NnEval>`.
	pub fn into_bot<const N: usize>(self) -> Box<dyn Bot<N>>
	where
		[(); N * N]:, {
		match self {
			InnerKind::ManualPlayer(p) => Box::new(p),
			InnerKind::RandomPlayer(p) => Box::new(p),
			InnerKind::GreedyForNumber(p) => Box::new(p),
			InnerKind::GreedyForScocre(p) => Box::new(p),
			InnerKind::Sadist(p) => Box::new(p),
			InnerKind::Rollout(p) => Box::new(p),
			InnerKind::OnnxPlayer(_) => panic!("OnnxPlayer cannot be constructed via into_bot; use GumbelBot + NnEval"),
		}
	}
}

/// A player: an algorithm optionally wrapped in a search.
///
/// `sims = None` → run the algorithm directly.
/// `sims = Some((Vanilla, n))` → wrap in vanilla UCT-MCTS with n simulations.
/// `sims = Some((Gumbel, n))` → wrap in Gumbel Sequential Halving with n simulations.
///
/// Display: `rollout`, `rollout|v800`, `rollout|g800`, `onnx:model_v5|g400`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerKind {
	pub inner: InnerKind,
	pub sims: Option<(SearchKind, u32)>,
}
impl PlayerKind {
	pub fn is_manual(&self) -> bool {
		self.inner.is_manual()
	}

	pub fn is_onnx(&self) -> bool {
		self.inner.is_onnx()
	}

	pub fn id(&self) -> Ustr {
		ustr(&self.to_string())
	}

	/// All non-Manual inner variants unwrapped, plus common vanilla-MCTS-wrapped rollout sims.
	pub fn defaults() -> Vec<PlayerKind> {
		let mut out: Vec<PlayerKind> = InnerKind::iter()
			.filter(|k| !k.is_manual() && !k.is_onnx())
			.map(|inner| PlayerKind { inner, sims: None })
			.collect();
		for sims in [50, 200, 800] {
			out.push(PlayerKind {
				inner: InnerKind::Rollout(Rollout {}),
				sims: Some((SearchKind::Vanilla, sims)),
			});
		}
		out
	}

	/// Construct a direct (unwrapped) `Bot<N>`. Panics if `sims.is_some()` or for OnnxPlayer.
	/// Prefer using `kind_into_bot` in the binary crate which handles search wrapping.
	pub fn into_bot<const N: usize>(self) -> Box<dyn Bot<N>>
	where
		[(); N * N]:, {
		assert!(self.sims.is_none(), "into_bot called on search-wrapped player; use kind_into_bot in the binary crate");
		self.inner.into_bot()
	}
}

impl std::fmt::Display for PlayerKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self.sims {
			None => write!(f, "{}", self.inner),
			Some((SearchKind::Vanilla, n)) => write!(f, "{}|v{n}", self.inner),
			Some((SearchKind::Gumbel, n)) => write!(f, "{}|g{n}", self.inner),
		}
	}
}

impl std::str::FromStr for PlayerKind {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		// Try stripping a trailing `|v<digits>` or `|g<digits>` suffix for sims.
		let (base, sims) = if let Some(pos) = s.rfind('|') {
			let suffix = &s[pos + 1..];
			let (kind, digits) = if let Some(rest) = suffix.strip_prefix('v') {
				(SearchKind::Vanilla, rest)
			} else if let Some(rest) = suffix.strip_prefix('g') {
				(SearchKind::Gumbel, rest)
			} else {
				return Err(s.to_string());
			};
			if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
				return Err(s.to_string());
			}
			let n: u32 = digits.parse().map_err(|e| format!("{e}"))?;
			(&s[..pos], Some((kind, n)))
		} else {
			(s, None)
		};

		let inner = base.parse::<InnerKind>().map_err(|_| s.to_string())?;
		Ok(Self { inner, sims })
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
