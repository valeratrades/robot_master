use std::fmt;

use rand::{Rng, seq::SliceRandom};

pub const MAX_CARD_VALUE: usize = 5;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CardValue(pub u8);

/// Fixed-size count array — O(1) lookup, no allocation, Copy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Hand {
	counts: [u8; MAX_CARD_VALUE + 1],
}

impl Hand {
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
}

impl fmt::Display for Hand {
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

pub fn new_deck(max_card: u8, nb_c: u8, rng: &mut impl Rng) -> Vec<CardValue> {
	let mut deck: Vec<CardValue> = (0..=max_card).flat_map(|v| std::iter::repeat_n(CardValue(v), nb_c as usize)).collect();
	deck.shuffle(rng);
	deck
}

pub fn deal(deck: &mut Vec<CardValue>, n: u8) -> Hand {
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
		let mut h = Hand::default();
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
		let mut h = Hand::default();
		h.take(CardValue(0));
	}

	#[test]
	fn deck_correct_counts() {
		let mut rng = SmallRng::seed_from_u64(42);
		let deck = new_deck(5, 6, &mut rng);
		assert_eq!(deck.len(), 36); // (5+1) * 6
		for v in 0..=5u8 {
			assert_eq!(deck.iter().filter(|&&c| c == CardValue(v)).count(), 6);
		}
	}

	#[test]
	fn deal_removes_from_deck() {
		let mut rng = SmallRng::seed_from_u64(0);
		let mut deck = new_deck(5, 6, &mut rng);
		let hand = deal(&mut deck, 12);
		assert_eq!(deck.len(), 24);
		let total: u8 = hand.counts.iter().sum();
		assert_eq!(total, 12);
	}
}
