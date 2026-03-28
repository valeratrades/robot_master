use robot_master_core::{
	game::{GameState, Move, PlayerId},
	scoring::victoire,
};
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::player::Player;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MatchResult {
	pub p1_id: Ustr,
	pub p2_id: Ustr,
	pub p1_score: u16,
	pub p2_score: u16,
	pub p1_weak_line: usize,
	pub p2_weak_line: usize,
	pub moves: Vec<SerMove>,
}

/// Serializable move (Pos and CardValue are not Serialize in core).
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct SerMove {
	pub row: u8,
	pub col: u8,
	pub card: u8,
}

impl From<Move> for SerMove {
	fn from(m: Move) -> Self {
		Self {
			row: m.pos.row,
			col: m.pos.col,
			card: m.card.0,
		}
	}
}

pub struct Match<const N: usize, P1: Player<N>, P2: Player<N>>
where
	[(); N * N]:, {
	game: GameState<N>,
	p1: P1,
	p2: P2,
	moves: Vec<Move>,
}

impl<const N: usize, P1: Player<N>, P2: Player<N>> Match<N, P1, P2>
where
	[(); N * N]:,
{
	pub fn new(game: GameState<N>, p1: P1, p2: P2) -> Self {
		Self { game, p1, p2, moves: Vec::new() }
	}

	/// Advance one turn: ask the current player for a move and apply it.
	///
	/// Returns `Ok(state)` if game continues, `Err(result)` if game just ended.
	///
	/// # Panics
	/// If the player returns an illegal move, or if called after the game is terminal.
	pub fn next(&mut self) -> Result<&GameState<N>, MatchResult> {
		assert!(!self.game.is_terminal(), "Match::next called on terminal game");

		let m = match self.game.turn {
			PlayerId::Cols => self.p1.choose_move(&self.game),
			PlayerId::Rows => self.p2.choose_move(&self.game),
		};

		self.game = self.game.apply_move(m).expect("illegal move in Match::next");
		self.moves.push(m);

		if self.game.is_terminal() { Err(self.build_result()) } else { Ok(&self.game) }
	}

	/// Play to completion.
	pub fn run(mut self) -> MatchResult {
		loop {
			match self.next() {
				Ok(_) => {}
				Err(result) => return result,
			}
		}
	}

	fn build_result(&self) -> MatchResult {
		let (s0, i0, s1, i1) = victoire(&self.game.board);
		MatchResult {
			p1_id: self.p1.id(),
			p2_id: self.p2.id(),
			p1_score: s0,
			p2_score: s1,
			p1_weak_line: i0,
			p2_weak_line: i1,
			moves: self.moves.iter().map(|&m| m.into()).collect(),
		}
	}
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng, seq::IteratorRandom};
	use robot_master_core::game::GameConfig;
	use ustr::ustr;

	use super::*;

	struct DummyRandom(SmallRng);
	impl Player<5> for DummyRandom {
		fn id(&self) -> Ustr {
			ustr("test-random")
		}

		fn choose_move(&mut self, game: &GameState<5>) -> Move {
			game.valid_moves().choose(&mut self.0).expect("no moves")
		}
	}

	#[test]
	fn match_runs_to_completion() {
		let mut rng = SmallRng::seed_from_u64(42);
		let game = GameState::new(GameConfig::default(), &mut rng);
		let p1 = DummyRandom(SmallRng::seed_from_u64(1));
		let p2 = DummyRandom(SmallRng::seed_from_u64(2));
		let m = Match::new(game, p1, p2);
		let result = m.run();
		assert_eq!(result.moves.len(), 24); // 5*5 - 1 center card
		assert_eq!(result.p1_id, ustr("test-random"));
	}

	#[test]
	fn match_next_step_by_step() {
		let mut rng = SmallRng::seed_from_u64(42);
		let game = GameState::new(GameConfig::default(), &mut rng);
		let p1 = DummyRandom(SmallRng::seed_from_u64(1));
		let p2 = DummyRandom(SmallRng::seed_from_u64(2));
		let mut m = Match::new(game, p1, p2);
		let mut steps = 0;
		loop {
			match m.next() {
				Ok(_) => steps += 1,
				Err(result) => {
					steps += 1;
					assert_eq!(steps, 24);
					assert_eq!(result.moves.len(), 24);
					break;
				}
			}
		}
	}
}
