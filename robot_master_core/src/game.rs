use std::fmt;

pub use board_game::board::Player;
use board_game::board::{BoardDone, BoardMoves, BoardSymmetry, PlayError};
use internal_iterator::InternalIterator;
use rand::Rng;
use secrecy::{ExposeSecret as _, SecretString};

use crate::{
	board::{Board, Pos},
	cards::{CardValue, Hand, deal, new_deck},
	scoring::victoire,
};

/// Player::A scores columns, Player::B scores rows.
#[inline]
pub fn scores_rows(p: Player) -> bool {
	p == Player::B
}

/// Display wrapper: prints `"Cols (A)"` / `"Rows (B)"` instead of bare `"A"` / `"B"`.
pub struct PlayerDisplay(pub Player);

impl fmt::Display for PlayerDisplay {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.0 {
			Player::A => f.write_str("P1 (Cols)"),
			Player::B => f.write_str("P2 (Rows)"),
		}
	}
}

impl fmt::Debug for PlayerDisplay {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(self, f)
	}
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GameConfig {
	/// board side size (can't be even)
	pub size: u8 = 5,
	/// whether the opponent's cards are hidden
	pub hide: bool = false,
}

impl GameConfig {
	/// Highest card value in play: `size`.
	pub fn max_card(self) -> u8 {
		self.size
	}

	/// Copies of each card value in the deck: `size + 1`.
	pub fn nb_c(self) -> u8 {
		self.size + 1
	}

	/// Number of cards each player receives: `(size² - 1) / 2`.
	pub fn cards_per_player(self) -> u8 {
		let n = self.size as u16;
		((n * n - 1) / 2) as u8
	}
}

#[derive(Clone, Copy, Debug, derive_more::Display, Eq, PartialEq)]
#[display("{}@({},{})", card.0, pos.row, pos.col)]
pub struct Move {
	pub pos: Pos,
	pub card: CardValue,
}

#[derive(Clone, Debug)]
pub struct GameState<const N: usize>
where
	[(); N * N]:,
	[(); N + 1]:, {
	pub board: Board<N>,
	pub turn: Player,

	hands: [Hand<N>; 2],
	hide: bool,
	player_secrets: [SecretString; 2],
}
impl PartialEq for GameState<const N: usize> {
	fn eq(&self, other: &Self) -> bool {
		self.board == other.board && self.turn == other.turn && self.hands == other.hands
	}
}

#[derive(Clone, Debug, derive_more::Deref)]
pub struct PlayerSigned {
	#[deref]
	pub player: Player,
	secret: SecretString = SecretString::new(Uuid::new_v7()),
}
impl PlayerSigned {
	pub fn new(player: Player) {
		Self { player, ..Default::default() }
	}
	pub fn check(&self, key: SecretString) -> bool {
		self.secret == key
	}
}

#[derive(derive_more::Display, miette::Display, thiserror::Error)]
pub enum GameError {
	UnauthorizedHandLookup,
}

impl<const N: usize> GameState<N>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	pub fn new(config: GameConfig, rng: &mut impl Rng, players: [PlayerSigned; 2]) -> Self {
		let mut deck = new_deck(N, rng);
		let mut board = Board::default();

		// Place center card (first off the deck, like Python's distribution_cartes).
		let center_card = deck.pop().expect("deck too small for center card");
		let center = Pos { row: N as u8 / 2, col: N as u8 / 2 };
		board.set(center, center_card.0);

		let cards_per_player = ((N * N - 1) / 2) as u8;
		let hand0 = deal(&mut deck, cards_per_player);
		let hand1 = deal(&mut deck, cards_per_player);

		Self {
			board,
			hands: [hand0, hand1],
			hide: config.hide,
			turn: Player::A,
			player_secrets: [players[0].secret, players[1].secret],
		}
	}

	pub fn hands(&self) -> Result<[Hand<N>;2], GameError> {
		match self.hide {
			true => Err(GameError::UnauthorizedHandLookup),
			false => Ok(self.hands),
		}
	}

	pub fn hand_signed(&self, player: Player, key: SecretString) -> Result<Hand<N>, GameError> {
		let player_idx = match *player {
			Player::A => 0,
			Player::B => 1,
		};
		if !self.hide {
			return Ok(self.hands[player_idx])
		}
		//Q: does .expose_secret() get optimized, knowing that secrets don't change, or should I forego pure secrecy, and just imply it with simple privately stored ids?
		match self.player_secrets[player_idx].expose_secret() == key.expose_secret() {
			true => Ok(self.hands[player_idx]),
			false => Err(GameError::UnauthorizedHandLookup),
		}
	}

	/// Total number of moves in a game (N*N cells minus the center card placed at setup).
	pub const fn total_moves() -> usize {
		N * N - 1
	}

	/// Standard `Iterator` over legal moves. Thin wrapper over the `Board` trait's `available_moves()`.
	///
	/// Use this when you need `.choose()`, `.next()`, or other std `Iterator` adapters.
	/// For push-based iteration (MCTS, NN), use `available_moves()` from `board_game::Board` directly.
	pub fn valid_moves(&self) -> impl Iterator<Item = Move> + '_ {
		let hand = &self.hands[self.turn.index() as usize];
		self.board.valid_placements().flat_map(move |pos| hand.iter_playable().map(move |card| Move { pos, card }))
	}
}

impl<const N: usize> fmt::Display for GameState<N>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"{}\n{} to play | {}: {} | {}: {}",
			self.board,
			PlayerDisplay(self.turn),
			PlayerDisplay(Player::A),
			self.hands[0],
			PlayerDisplay(Player::B),
			self.hands[1]
		)
	}
}

impl<const N: usize> board_game::board::Board for GameState<N>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	type Move = Move;

	fn next_player(&self) -> Player {
		self.turn
	}

	fn is_available_move(&self, mv: Self::Move) -> Result<bool, BoardDone> {
		if self.board.is_full() {
			return Err(BoardDone);
		}
		let hand = &self.hands[self.turn.index() as usize];
		Ok(self.board.is_playable(mv.pos) && hand.count(mv.card) > 0)
	}

	fn play(&mut self, mv: Self::Move) -> Result<(), PlayError> {
		if self.board.is_full() {
			return Err(PlayError::BoardDone);
		}
		if !self.board.is_playable(mv.pos) || self.hands[self.turn.index() as usize].count(mv.card) == 0 {
			return Err(PlayError::UnavailableMove);
		}
		self.board.set(mv.pos, mv.card.0);
		self.hands[self.turn.index() as usize].take(mv.card);
		self.turn = self.turn.other();
		Ok(())
	}

	fn outcome(&self) -> Option<board_game::board::Outcome> {
		if !self.board.is_full() {
			return None;
		}
		let (s0, _, s1, _) = victoire(&self.board);
		Some(match s0.cmp(&s1) {
			std::cmp::Ordering::Greater => board_game::board::Outcome::WonBy(Player::A),
			std::cmp::Ordering::Less => board_game::board::Outcome::WonBy(Player::B),
			std::cmp::Ordering::Equal => board_game::board::Outcome::Draw,
		})
	}

	fn can_lose_after_move() -> bool {
		true
	}
}

impl<'a, const N: usize> BoardMoves<'a, GameState<N>> for GameState<N>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	type AllMovesIterator = AllMoves<N>;
	type AvailableMovesIterator = AvailableMoves<'a, N>;

	fn all_possible_moves() -> Self::AllMovesIterator {
		AllMoves
	}

	fn available_moves(&'a self) -> Result<Self::AvailableMovesIterator, BoardDone> {
		if self.board.is_full() {
			return Err(BoardDone);
		}
		Ok(AvailableMoves { state: self })
	}
}

/// Iterator over all theoretically possible moves: every (pos, card) pair on an NxN board.
#[derive(Clone)]
pub struct AllMoves<const N: usize>;

impl<const N: usize> InternalIterator for AllMoves<N>
where
	[(); N * N]:,
{
	type Item = Move;

	fn try_for_each<T, F>(self, mut f: F) -> std::ops::ControlFlow<T>
	where
		F: FnMut(Self::Item) -> std::ops::ControlFlow<T>, {
		for row in 0..N as u8 {
			for col in 0..N as u8 {
				for card in 0..=N as u8 {
					f(Move {
						pos: Pos { row, col },
						card: CardValue(card),
					})?;
				}
			}
		}
		std::ops::ControlFlow::Continue(())
	}
}

/// Iterator over currently available moves for a given game state.
#[derive(Clone)]
pub struct AvailableMoves<'a, const N: usize>
where
	[(); N * N]:,
	[(); N + 1]:, {
	state: &'a GameState<N>,
}

impl<'a, const N: usize> InternalIterator for AvailableMoves<'a, N>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	type Item = Move;

	fn try_for_each<T, F>(self, mut f: F) -> std::ops::ControlFlow<T>
	where
		F: FnMut(Self::Item) -> std::ops::ControlFlow<T>, {
		let hand = &self.state.hands[self.state.turn.index() as usize];
		for pos in self.state.board.valid_placements() {
			for card in hand.iter_playable() {
				f(Move { pos, card })?;
			}
		}
		std::ops::ControlFlow::Continue(())
	}
}

impl<const N: usize> BoardSymmetry<GameState<N>> for GameState<N>
where
	[(); N * N]:,
	[(); N + 1]:,
{
	type CanonicalKey = ();
	type Symmetry = board_game::symmetry::UnitSymmetry;

	fn map(&self, _: Self::Symmetry) -> Self {
		self.clone()
	}

	fn map_move(&self, _: Self::Symmetry, mv: Move) -> Move {
		mv
	}

	fn canonical_key(&self) -> Self::CanonicalKey {}
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
	fn board_trait_play_and_outcome() {
		use board_game::board::Board as _;
		let mut s = state5();
		let first_move = s.valid_moves().next().unwrap();
		s.play(first_move).unwrap();
		assert_eq!(s.next_player(), Player::B);
		assert!(s.outcome().is_none());

		// play to completion using valid_moves
		while s.outcome().is_none() {
			let mv = s.valid_moves().next().unwrap();
			s.play(mv).unwrap();
		}
		assert!(s.is_done());
		assert!(s.outcome().is_some());
	}

	#[test]
	fn board_trait_available_moves_count() {
		use internal_iterator::InternalIterator;
		let s = state5();
		let trait_count = s.available_moves().unwrap().count();
		let direct_count = s.valid_moves().count();
		assert_eq!(trait_count, direct_count);
	}

	#[test]
	fn play_valid() {
		use board_game::board::Board as _;
		let s = state5();
		let m = s.valid_moves().next().expect("no valid moves at start");
		let next = s.clone_and_play(m).expect("valid move rejected");
		assert_eq!(next.turn, Player::B);
		assert!(!next.board.is_empty(m.pos));
	}

	#[test]
	fn play_invalid_pos() {
		use board_game::board::Board as _;
		let s = state5();
		let m = Move {
			pos: Pos { row: 0, col: 0 },
			card: CardValue(0),
		};
		assert!(matches!(s.clone_and_play(m), Err(board_game::board::PlayError::UnavailableMove)));
	}

	#[test]
	fn play_card_not_in_hand() {
		use board_game::board::Board as _;
		let mut rng = SmallRng::seed_from_u64(7);
		let mut s: GameState<5> = GameState::new(GameConfig::default(), &mut rng);
		while s.hands[0].count(CardValue(5)) > 0 {
			s.hands[0].take(CardValue(5));
		}
		let pos = s.board.valid_placements().next().unwrap();
		let m = Move { pos, card: CardValue(5) };
		assert!(matches!(s.clone_and_play(m), Err(board_game::board::PlayError::UnavailableMove)));
	}
}
