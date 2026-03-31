use std::str::FromStr;

use robot_master_core::game::{GameState, Move};

use crate::algos::validate_manual_name;

/// Something that can decide which move to play given a game state.
///
/// # Contract
/// - `choose_move` must return a legal move. If it doesn't, `Match` will panic (fail-fast).
/// - For bots that don't make autonomous decisions (e.g. manual/human input), `choose_move`
///   should panic — the interface must provide moves externally via `Match::next(Some(m))`.
pub trait Bot<const N: usize>: Send + Sync
where
	[(); N * N]:, {
	/// Pick a move given the current game state.
	fn choose_move(&mut self, game: &GameState<N>) -> Move;
}

/// Blanket impl so `Box<dyn Bot<N>>` is itself a `Bot<N>`.
/// Needed for dynamic dispatch contexts (TUI, tournament).
impl<const N: usize> Bot<N> for Box<dyn Bot<N>>
where
	[(); N * N]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		(**self).choose_move(game)
	}
}

/// Placeholder for human-controlled players.
///
/// `choose_move` panics — the caller must always supply moves via `Match::next(Some(m))`.
#[derive(Clone, Debug, derive_more::Display, Eq, PartialEq)]
#[display("manual:{name}")]
pub struct ManualPlayer {
	pub name: String,
}

impl Default for ManualPlayer {
	fn default() -> Self {
		Self { name: "Player".into() }
	}
}

impl FromStr for ManualPlayer {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let lower = s.to_lowercase();
		if let Some(name) = lower.strip_prefix("manual:") {
			validate_manual_name(name)?;
			return Ok(Self { name: name.to_string() });
		}
		if lower == "manual" {
			return Ok(Self::default());
		}
		Err(s.to_string())
	}
}

impl<const N: usize> Bot<N> for ManualPlayer
where
	[(); N * N]:,
{
	fn choose_move(&mut self, _game: &GameState<N>) -> Move {
		panic!("ManualPlayer::choose_move called — caller must supply moves via Match::next(Some(m))")
	}
}

/// Forwarding impl so `&mut dyn Bot<N>` is itself a `Bot<N>`.
/// Needed for tournament where players are borrowed from a slice.
impl<const N: usize> Bot<N> for &mut dyn Bot<N>
where
	[(); N * N]:,
{
	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		(**self).choose_move(game)
	}
}
