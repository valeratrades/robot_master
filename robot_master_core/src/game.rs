use rand::Rng;
use thiserror::Error;

use crate::{
	board::{Board, Pos},
	cards::{CardValue, Hand, deal, new_deck},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlayerId {
	/// Player one - scores cols
	Cols,
	/// Player two - scores rows.
	Rows,
}

impl PlayerId {
	pub fn opponent(self) -> Self {
		match self {
			PlayerId::Cols => PlayerId::Rows,
			PlayerId::Rows => PlayerId::Cols,
		}
	}

	//Q: not sure if I should keep this now, that I renamed players themselves to match
	#[inline]
	pub fn scores_rows(self) -> bool {
		self == PlayerId::Rows
	}
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GameConfig {
	pub size: u8 = 5,
	pub max_card: u8 = 5,
	pub nb_c: u8 = 6,
	pub cards_dealt: u8 = 12,
}

#[derive(Clone, Copy, Debug)]
pub struct Move {
	pub pos: Pos,
	pub card: CardValue,
}

#[derive(Debug, Error)]
pub enum MoveError {
	#[error("position {0:?} is not a valid placement")]
	InvalidPosition(Pos),
	#[error("card {0:?} is not in hand")]
	CardNotInHand(CardValue),
}

#[derive(Clone, Copy, Debug)]
pub struct GameState<const N: usize>
where
	[(); N * N]:, {
	pub board: Board<N>,
	pub hands: [Hand; 2],
	pub turn: PlayerId,
	pub config: GameConfig,
}

impl<const N: usize> GameState<N>
where
	[(); N * N]:,
{
	pub fn new(config: GameConfig, rng: &mut impl Rng) -> Self {
		let mut deck = new_deck(config.max_card, config.nb_c, rng);
		let mut board = Board::default();

		// Place center card (first off the deck, like Python's distribution_cartes).
		let center_card = deck.pop().expect("deck too small for center card");
		let center = Pos { row: N as u8 / 2, col: N as u8 / 2 };
		board.set(center, center_card.0);

		let hand0 = deal(&mut deck, config.cards_dealt);
		let hand1 = deal(&mut deck, config.cards_dealt);

		Self {
			board,
			hands: [hand0, hand1],
			turn: PlayerId::Cols,
			config,
		}
	}

	pub fn valid_moves(&self) -> impl Iterator<Item = Move> + '_ {
		let hand = &self.hands[self.turn as usize];
		self.board.valid_placements().flat_map(move |pos| hand.iter_playable().map(move |card| Move { pos, card }))
	}

	/// Returns a new GameState with the move applied, or an error if the move is invalid.
	pub fn apply_move(&self, m: Move) -> Result<Self, MoveError> {
		if !self.board.is_playable(m.pos) {
			return Err(MoveError::InvalidPosition(m.pos));
		}
		let hand = &self.hands[self.turn as usize];
		if hand.count(m.card) == 0 {
			return Err(MoveError::CardNotInHand(m.card));
		}

		let mut next = *self;
		next.board.set(m.pos, m.card.0);
		next.hands[self.turn as usize].take(m.card);
		next.turn = self.turn.opponent();
		Ok(next)
	}

	pub fn is_terminal(&self) -> bool {
		self.board.is_full()
	}
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng};

	use super::*;

	fn state5() -> GameState<5> {
		let mut rng = SmallRng::seed_from_u64(7);
		GameState::new(GameConfig::default(), &mut rng)
	}

	#[test]
	fn apply_move_valid() {
		let s = state5();
		let m = s.valid_moves().next().expect("no valid moves at start");
		let next = s.apply_move(m).expect("valid move rejected");
		assert_eq!(next.turn, PlayerId::Rows);
		assert!(!next.board.is_empty(m.pos));
	}

	#[test]
	fn apply_move_invalid_pos() {
		let s = state5();
		let m = Move {
			pos: Pos { row: 0, col: 0 },
			card: CardValue(0),
		};
		assert!(matches!(s.apply_move(m), Err(MoveError::InvalidPosition(_))));
	}

	#[test]
	fn apply_move_card_not_in_hand() {
		// Build a state with a hand that is missing card value 5.
		let mut rng = SmallRng::seed_from_u64(7);
		let mut s: GameState<5> = GameState::new(GameConfig::default(), &mut rng);
		// Drain card 5 from P0's hand by returning it conceptually — just set count to 0 directly
		// via take() calls. If hand has some 5s, drain them; if not, it's already missing.
		while s.hands[0].count(CardValue(5)) > 0 {
			s.hands[0].take(CardValue(5));
		}
		let pos = s.board.valid_placements().next().unwrap();
		let m = Move { pos, card: CardValue(5) };
		assert!(matches!(s.apply_move(m), Err(MoveError::CardNotInHand(_))));
	}
}
