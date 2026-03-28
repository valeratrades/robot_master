use robot_master_core::{
	game::{GameState, Move, PlayerId},
	scoring::victoire,
};
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::{
	db::RatingDb,
	player::Player,
	rating::{self, Outcome},
};

#[derive(Clone, Debug)]
pub struct EloUpdate {
	pub p1_old: f64,
	pub p1_new: f64,
	pub p2_old: f64,
	pub p2_new: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MatchResult {
	pub p1_id: Ustr,
	pub p2_id: Ustr,
	pub p1_score: u16,
	pub p2_score: u16,
	pub p1_weak_line: usize,
	pub p2_weak_line: usize,
	pub moves: Vec<SerMove>,
	#[serde(skip)]
	pub elo_update: Option<EloUpdate>,
}
impl MatchResult {
	/// Perform elo update against the given db, populating `self.elo_update`.
	pub fn update_elo(&mut self, rating_db: &dyn RatingDb) {
		let mut ratings = rating_db.load_ratings();

		let outcome = match self.p1_score.cmp(&self.p2_score) {
			std::cmp::Ordering::Greater => Outcome::P1Win,
			std::cmp::Ordering::Less => Outcome::P2Win,
			std::cmp::Ordering::Equal => Outcome::Draw,
		};

		let r1 = ratings.entry(self.p1_id).or_default().clone();
		let r2 = ratings.entry(self.p2_id).or_default().clone();

		let (new_r1, new_r2) = rating::elo_update(&r1, &r2, outcome, rating::DEFAULT_K);
		let update = EloUpdate {
			p1_old: r1.rating,
			p1_new: new_r1.rating,
			p2_old: r2.rating,
			p2_new: new_r2.rating,
		};

		ratings.insert(self.p1_id, new_r1);
		ratings.insert(self.p2_id, new_r2);
		rating_db.save_ratings(&ratings);

		self.elo_update = Some(update);
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

pub struct Match<const N: usize, P1: Player<N>, P2: Player<N>>
where
	[(); N * N]:, {
	game: GameState<N>,
	p1: P1,
	p2: P2,
	moves: Vec<Move>,
	rating_db: Option<Box<dyn RatingDb>>,
}

impl<const N: usize, P1: Player<N>, P2: Player<N>> Match<N, P1, P2>
where
	[(); N * N]:,
{
	pub fn new(game: GameState<N>, p1: P1, p2: P2) -> Self {
		Self {
			game,
			p1,
			p2,
			moves: Vec::new(),
			rating_db: None,
		}
	}

	pub fn with_rating_db(mut self, db: Box<dyn RatingDb>) -> Self {
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
	/// Returns `Ok(state)` if game continues, `Err(result)` if game just ended.
	///
	/// # Panics
	/// If the move is illegal, or if called after the game is terminal.
	pub fn next(&mut self, external_move: Option<Move>) -> Result<&GameState<N>, MatchResult> {
		assert!(!self.game.is_terminal(), "Match::next called on terminal game");

		let m = external_move.unwrap_or_else(|| match self.game.turn {
			PlayerId::Cols => self.p1.choose_move(&self.game),
			PlayerId::Rows => self.p2.choose_move(&self.game),
		});

		self.game = self.game.apply_move(m).expect("illegal move in Match::next");
		self.moves.push(m);

		if self.game.is_terminal() {
			let mut result = self.build_result();
			if let Some(ref db) = self.rating_db {
				result.update_elo(db.as_ref());
			}
			Err(result)
		} else {
			Ok(&self.game)
		}
	}

	/// Play to completion (all players must be AI).
	pub fn run(mut self) -> MatchResult {
		loop {
			match self.next(None) {
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
			elo_update: None,
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
			match m.next(None) {
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
