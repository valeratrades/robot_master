use robot_master_core::game::{GameState, Move};
use ustr::Ustr;

/// Something that can decide which move to play given a game state.
///
/// # Contract
/// - `choose_move` must return a legal move. If it doesn't, `Match` will panic (fail-fast).
/// - For players that don't make autonomous decisions (e.g. manual/human input), `choose_move`
///   should panic — the interface must provide moves externally via `Match::next(Some(m))`.
pub trait Player<const N: usize>
where
	[(); N * N]:, {
	/// Stable identifier used for Elo tracking, display, serialization.
	fn id(&self) -> Ustr;

	/// Pick a move given the current game state.
	fn choose_move(&mut self, game: &GameState<N>) -> Move;
}

/// Blanket impl so `Box<dyn Player<N>>` is itself a `Player<N>`.
/// Needed for dynamic dispatch contexts (TUI, tournament).
impl<const N: usize> Player<N> for Box<dyn Player<N>>
where
	[(); N * N]:,
{
	fn id(&self) -> Ustr {
		(**self).id()
	}

	fn choose_move(&mut self, game: &GameState<N>) -> Move {
		(**self).choose_move(game)
	}
}
