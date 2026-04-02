use std::{ops::ControlFlow, sync::Arc};

use board_game::board::Board as _;
use robot_master_core::{
	board::{Cell, Pos},
	game::{GameState, Move, Player},
	scoring::victoire,
};
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::{
	db::RatingDb,
	player::Bot,
	rating::{self, Outcome, Rating},
};

/// Type-erased match interface for use in contexts (e.g. Bevy ECS) that can't be const-generic.
pub trait DynMatch {
	fn size(&self) -> u8;
	fn get(&self, pos: Pos) -> Cell;
	fn is_playable(&self, pos: Pos) -> bool;
	fn is_done(&self) -> bool;
	fn turn(&self) -> Player;
	fn hands(&self) -> [Vec<u8>; 2];
	fn next(&mut self, external_move: Option<Move>) -> ControlFlow<MatchResult>;
	/// (p1_score, p1_weak_line, p2_score, p2_weak_line)
	fn scores(&self) -> (u16, usize, u16, usize);
}
#[derive(Clone, Debug)]
pub struct RatingUpdate {
	pub p1_old: Rating,
	pub p1_new: Rating,
	pub p2_old: Rating,
	pub p2_new: Rating,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct MatchResult {
	pub p1_id: Ustr,
	pub p2_id: Ustr,
	pub p1_score: u16,
	pub p2_score: u16,
	pub p1_weak_line: usize,
	pub p2_weak_line: usize,
	pub moves: Vec<SerMove>,
	#[serde(skip)]
	pub rating_update: Option<RatingUpdate>,
	/// If set, `Drop` will automatically persist the Glicko-2 update to this db.
	#[serde(skip)]
	rating_db: Option<Arc<dyn RatingDb>>,
}
impl MatchResult {
	pub fn new(p1_id: Ustr, p2_id: Ustr, p1_score: u16, p2_score: u16, p1_weak_line: usize, p2_weak_line: usize, moves: Vec<SerMove>, rating_db: Option<Arc<dyn RatingDb>>) -> Self {
		Self {
			p1_id,
			p2_id,
			p1_score,
			p2_score,
			p1_weak_line,
			p2_weak_line,
			moves,
			rating_update: None,
			rating_db,
		}
	}

	/// Immediately compute and persist the Glicko-2 update, returning it.
	/// Same rating update and save as happens on Drop, but you get the value back
	pub fn commit(mut self) -> RatingUpdate {
		let db = self.rating_db.take().expect("MatchResult::commit called without a rating_db set");
		self.update_rating(db.as_ref());
		let update = self.rating_update.take().expect("update_rating must populate rating_update");
		std::mem::forget(self);
		update
	}

	/// Consume `self` without saving ratings. Use inside tournament where ratings are
	/// managed explicitly in-memory and saved once at the end.
	// here to explicitly document this as a valid pattern
	pub fn forget(self) {
		std::mem::forget(self);
	}

	/// Perform Glicko-2 rating update against the given db, populating `self.rating_update`.
	fn update_rating(&mut self, rating_db: &dyn RatingDb) {
		let mut ratings = rating_db.load_ratings();

		let outcome = match self.p1_score.cmp(&self.p2_score) {
			std::cmp::Ordering::Greater => Outcome::P1Win,
			std::cmp::Ordering::Less => Outcome::P2Win,
			std::cmp::Ordering::Equal => Outcome::Draw,
		};

		let r1 = ratings.entry(self.p1_id).or_default().clone();
		let r2 = ratings.entry(self.p2_id).or_default().clone();

		let (new_r1, new_r2) = rating::glicko_update(&r1, &r2, outcome);
		let update = RatingUpdate {
			p1_old: r1.clone(),
			p1_new: new_r1.clone(),
			p2_old: r2.clone(),
			p2_new: new_r2.clone(),
		};

		ratings.insert(self.p1_id, new_r1);
		ratings.insert(self.p2_id, new_r2);
		rating_db.save_ratings(&ratings);

		self.rating_update = Some(update);
	}
}

impl std::fmt::Debug for MatchResult {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MatchResult")
			.field("p1_id", &self.p1_id)
			.field("p2_id", &self.p2_id)
			.field("p1_score", &self.p1_score)
			.field("p2_score", &self.p2_score)
			.field("p1_weak_line", &self.p1_weak_line)
			.field("p2_weak_line", &self.p2_weak_line)
			.field("moves", &self.moves)
			.field("rating_update", &self.rating_update)
			.finish_non_exhaustive()
	}
}

impl Drop for MatchResult {
	fn drop(&mut self) {
		if let Some(ref db) = self.rating_db.take() {
			self.update_rating(db.as_ref());
		}
	}
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

pub struct Match<const N: usize, P1: Bot<N>, P2: Bot<N>>
where
	[(); N * N]:,
	[(); N + 1]:, {
	game: GameState<N>,
	p1: P1,
	p2: P2,
	p1_id: Ustr,
	p2_id: Ustr,
	moves: Vec<Move>,
	rating_db: Option<Arc<dyn RatingDb>>,
}

impl<const N: usize, P1: Bot<N>, P2: Bot<N>> Match<N, P1, P2>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	pub fn new(game: GameState<N>, p1: P1, p2: P2, p1_id: Ustr, p2_id: Ustr) -> Self {
		Self {
			game,
			p1,
			p2,
			p1_id,
			p2_id,
			moves: Vec::new(),
			rating_db: None,
		}
	}

	pub fn with_rating_db(mut self, db: Arc<dyn RatingDb>) -> Self {
		self.rating_db = Some(db);
		self
	}

	pub fn game(&self) -> &GameState<N> {
		&self.game
	}

	/// Advance one turn and apply the given move (or ask the current player).
	///
	/// Pass `Some(m)` to supply a move externally (manual/human input).
	/// Pass `None` to let the current player's `choose_move` decide.
	///
	/// Returns `Continue(state)` if game continues, `Break(result)` if game just ended.
	///
	/// # Panics
	/// If the move is illegal, or if called after the game is terminal.
	pub fn next(&mut self, external_move: Option<Move>) -> ControlFlow<MatchResult, &GameState<N>> {
		assert!(!self.game.is_done(), "Match::next called on terminal game");

		let m = external_move.unwrap_or_else(|| match self.game.turn {
			Player::A => self.p1.choose_move(&self.game),
			Player::B => self.p2.choose_move(&self.game),
		});

		self.game = self.game.clone_and_play(m).expect("illegal move in Match::next");
		self.moves.push(m);

		if self.game.is_done() {
			ControlFlow::Break(self.build_result())
		} else {
			ControlFlow::Continue(&self.game)
		}
	}

	/// Play to completion (all players must be AI).
	pub fn run(mut self) -> MatchResult {
		for _ in 0..GameState::<N>::total_moves() {
			if let ControlFlow::Break(result) = self.next(None) {
				return result;
			}
		}
		panic!("game did not terminate within {} moves", GameState::<N>::total_moves());
	}

	fn build_result(&self) -> MatchResult {
		let (s0, i0, s1, i1) = victoire(&self.game.board);
		MatchResult {
			p1_id: self.p1_id,
			p2_id: self.p2_id,
			p1_score: s0,
			p2_score: s1,
			p1_weak_line: i0,
			p2_weak_line: i1,
			moves: self.moves.iter().map(|&m| m.into()).collect(),
			rating_update: None,
			rating_db: self.rating_db.as_ref().map(|db| db.clone()),
		}
	}
}

impl<const N: usize, P1: Bot<N>, P2: Bot<N>> DynMatch for Match<N, P1, P2>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn size(&self) -> u8 {
		N as u8
	}

	fn get(&self, pos: Pos) -> Cell {
		self.game.board.get(pos)
	}

	fn is_playable(&self, pos: Pos) -> bool {
		self.game.board.is_playable(pos)
	}

	fn is_done(&self) -> bool {
		self.game.is_done()
	}

	fn turn(&self) -> Player {
		self.game.turn
	}

	fn hands(&self) -> [Vec<u8>; 2] {
		[self.game.hands[0].to_counts_vec(), self.game.hands[1].to_counts_vec()]
	}

	fn next(&mut self, external_move: Option<Move>) -> ControlFlow<MatchResult> {
		match Match::next(self, external_move) {
			ControlFlow::Continue(_) => ControlFlow::Continue(()),
			ControlFlow::Break(result) => ControlFlow::Break(result),
		}
	}

	fn scores(&self) -> (u16, usize, u16, usize) {
		victoire(&self.game.board)
	}
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng, seq::IteratorRandom};
	use robot_master_core::game::GameConfig;
	use ustr::ustr;

	use super::*;

	struct DummyRandom(SmallRng);
	impl Bot<5> for DummyRandom {
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
		let m = Match::new(game, p1, p2, ustr("p1"), ustr("p2"));
		let result = m.run();
		assert_eq!(result.moves.len(), 24); // 5*5 - 1 center card
		assert_eq!(result.p1_id, ustr("p1"));
	}

	#[test]
	fn match_next_step_by_step() {
		let mut rng = SmallRng::seed_from_u64(42);
		let game = GameState::new(GameConfig::default(), &mut rng);
		let p1 = DummyRandom(SmallRng::seed_from_u64(1));
		let p2 = DummyRandom(SmallRng::seed_from_u64(2));
		let mut m = Match::new(game, p1, p2, ustr("p1"), ustr("p2"));
		let mut steps = 0;
		while let ControlFlow::Continue(_) = m.next(None) {
			steps += 1;
		}
		// The final move that ended the game
		steps += 1;
		assert_eq!(steps, 24);
	}
}
