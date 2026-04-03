use std::fmt;

use rand::{Rng, seq::SliceRandom};

/// Maximum supported board size (and max card value), capped at u4::MAX = 15.
pub const MAX_BOARD_SIZE: usize = 15;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CardValue(pub u8);

/// Fixed-size count array — O(1) lookup, no allocation, Copy.
/// N is the board size; card values run 0..=N, so the array is N+1 slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hand<const N: usize>
where
	[(); N + 1]:, {
	counts: [u8; N + 1],
}
impl<const N: usize> Hand<N>
where
	[(); N + 1]:,
{
	#[inline]
	pub fn count(&self, v: CardValue) -> u8 {
		self.counts[v.0 as usize]
	}

	/// Panics if the card is not in hand — caller must verify first.
	#[inline]
	pub fn take(&mut self, v: CardValue) {
		let c = &mut self.counts[v.0 as usize];
		assert!(*c > 0, "tried to take card {v:?} not in hand");
		*c -= 1;
	}

	#[inline]
	pub fn put(&mut self, v: CardValue) {
		self.counts[v.0 as usize] += 1;
	}

	pub fn iter_playable(&self) -> impl Iterator<Item = CardValue> + '_ {
		self.counts.iter().enumerate().filter(|&(_, &c)| c > 0).map(|(v, _)| CardValue(v as u8))
	}

	pub fn is_empty(&self) -> bool {
		self.counts.iter().all(|&c| c == 0)
	}

	pub fn total(&self) -> u8 {
		self.counts.iter().sum()
	}

	/// Returns counts as a plain vec, indexed by card value.
	pub fn to_counts_vec(&self) -> Vec<u8> {
		self.counts.to_vec()
	}
}

impl<const N: usize> Default for Hand<N>
where
	[(); N + 1]:,
{
	fn default() -> Self {
		Self { counts: [0; N + 1] }
	}
}

impl<const N: usize> fmt::Display for Hand<N>
where
	[(); N + 1]:,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{{")?;
		let mut first = true;
		for (v, &c) in self.counts.iter().enumerate() {
			if c > 0 {
				if !first {
					write!(f, ", ")?;
				}
				write!(f, "{v}:{c}")?;
				first = false;
			}
		}
		write!(f, "}}")
	}
}

/// Build a deck for an N×N board: values 0..=N, each appearing N+1 times, shuffled.
/// Total deck size: (N+1)². For 5×5: values 0-5, 6 copies each = 36 cards.
pub fn new_deck(n: usize, rng: &mut impl Rng) -> Vec<CardValue> {
	let np1 = n + 1;
	let mut deck: Vec<CardValue> = (0..np1).flat_map(|v| std::iter::repeat_n(CardValue(v as u8), np1)).collect();
	deck.shuffle(rng);
	deck
}

pub fn deal<const N: usize>(deck: &mut Vec<CardValue>, n: u8) -> Hand<N>
where
	[(); N + 1]:, {
	let mut hand = Hand::default();
	for _ in 0..n {
		let card = deck.pop().expect("deck exhausted during deal");
		hand.put(card);
	}
	hand
}

#[cfg(test)]
mod tests {
	use rand::{SeedableRng, rngs::SmallRng};

	use super::*;

	#[test]
	fn hand_take_put() {
		let mut h = Hand::<5>::default();
		h.put(CardValue(3));
		h.put(CardValue(3));
		assert_eq!(h.count(CardValue(3)), 2);
		h.take(CardValue(3));
		assert_eq!(h.count(CardValue(3)), 1);
		assert!(!h.is_empty());
		h.take(CardValue(3));
		assert!(h.is_empty());
	}

	#[test]
	#[should_panic]
	fn hand_take_absent_panics() {
		let mut h = Hand::<5>::default();
		h.take(CardValue(0));
	}

	#[test]
	fn deck_correct_counts_5x5() {
		let mut rng = SmallRng::seed_from_u64(42);
		let deck = new_deck(5, &mut rng);
		assert_eq!(deck.len(), 36); // (5+1)² = 36
		for v in 0..=5u8 {
			assert_eq!(deck.iter().filter(|&&c| c == CardValue(v)).count(), 6);
		}
	}

	#[test]
	fn deck_correct_counts_7x7() {
		let mut rng = SmallRng::seed_from_u64(42);
		let deck = new_deck(7, &mut rng);
		assert_eq!(deck.len(), 64); // (7+1)² = 64
		for v in 0..=7u8 {
			assert_eq!(deck.iter().filter(|&&c| c == CardValue(v)).count(), 8);
		}
	}

	#[test]
	fn deal_removes_from_deck() {
		let mut rng = SmallRng::seed_from_u64(0);
		let mut deck = new_deck(5, &mut rng);
		let hand: Hand<5> = deal(&mut deck, 12);
		assert_eq!(deck.len(), 24); // 36 - 12
		let total: u8 = hand.counts.iter().sum();
		assert_eq!(total, 12);
	}
}
