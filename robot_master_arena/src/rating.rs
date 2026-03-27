use serde::{Deserialize, Serialize};

pub const DEFAULT_RATING: f64 = 1500.0;
pub const DEFAULT_K: f64 = 32.0;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EloRating {
	pub rating: f64,
	pub games_played: u32,
}

impl Default for EloRating {
	fn default() -> Self {
		Self {
			rating: DEFAULT_RATING,
			games_played: 0,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub enum Outcome {
	P1Win,
	P2Win,
	Draw,
}

/// Pure Elo update. Returns new ratings for both players.
pub fn elo_update(r1: &EloRating, r2: &EloRating, outcome: Outcome, k: f64) -> (EloRating, EloRating) {
	let expected_1 = 1.0 / (1.0 + 10f64.powf((r2.rating - r1.rating) / 400.0));
	let expected_2 = 1.0 - expected_1;

	let (actual_1, actual_2) = match outcome {
		Outcome::P1Win => (1.0, 0.0),
		Outcome::P2Win => (0.0, 1.0),
		Outcome::Draw => (0.5, 0.5),
	};

	(
		EloRating {
			rating: r1.rating + k * (actual_1 - expected_1),
			games_played: r1.games_played + 1,
		},
		EloRating {
			rating: r2.rating + k * (actual_2 - expected_2),
			games_played: r2.games_played + 1,
		},
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn equal_ratings_draw_no_change() {
		let r1 = EloRating::default();
		let r2 = EloRating::default();
		let (new1, new2) = elo_update(&r1, &r2, Outcome::Draw, DEFAULT_K);
		assert!((new1.rating - 1500.0).abs() < 0.01);
		assert!((new2.rating - 1500.0).abs() < 0.01);
	}

	#[test]
	fn winner_gains_loser_loses() {
		let r1 = EloRating::default();
		let r2 = EloRating::default();
		let (new1, new2) = elo_update(&r1, &r2, Outcome::P1Win, DEFAULT_K);
		assert!(new1.rating > 1500.0);
		assert!(new2.rating < 1500.0);
		// Elo is zero-sum
		assert!((new1.rating + new2.rating - 3000.0).abs() < 0.01);
	}

	#[test]
	fn upset_gives_bigger_change() {
		let strong = EloRating { rating: 1800.0, games_played: 100 };
		let weak = EloRating { rating: 1200.0, games_played: 100 };
		// Weak player wins — upset
		let (new_strong, new_weak) = elo_update(&strong, &weak, Outcome::P2Win, DEFAULT_K);
		let strong_loss = strong.rating - new_strong.rating;
		// Expected: strong player loses a lot (close to k)
		assert!(strong_loss > 25.0);

		// Now strong wins — expected result
		let (new_strong2, _) = elo_update(&strong, &weak, Outcome::P1Win, DEFAULT_K);
		let strong_gain = new_strong2.rating - strong.rating;
		// Expected: strong player gains little
		assert!(strong_gain < 7.0);

		let _ = new_weak; // suppress unused
	}

	#[test]
	fn games_played_increments() {
		let r1 = EloRating::default();
		let r2 = EloRating::default();
		let (new1, new2) = elo_update(&r1, &r2, Outcome::P1Win, DEFAULT_K);
		assert_eq!(new1.games_played, 1);
		assert_eq!(new2.games_played, 1);
	}
}
