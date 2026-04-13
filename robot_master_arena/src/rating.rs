/// Lichess-derived Glicko-2 parameters.
const TAU: f64 = 0.75;
const CONVERGENCE_TOLERANCE: f64 = 1e-6;
const MAX_ITERATIONS: usize = 1000;
/// Glicko-1 ↔ Glicko-2 scale factor.
const SCALE: f64 = 173.7178;

const MIN_RATING: f64 = 400.0;
const MAX_RATING: f64 = 4000.0;
const MIN_DEVIATION: f64 = 45.0;
const MAX_DEVIATION: f64 = 500.0;
const MAX_VOLATILITY: f64 = 0.1;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Rating {
	pub rating: f64,
	pub deviation: f64,
	pub volatility: f64,
}
impl Rating {
	/// Whether this player is still provisional (high uncertainty).
	pub fn is_provisional(&self) -> bool {
		self.deviation >= 110.0
	}

	fn to_glicko2(&self) -> (f64, f64) {
		((self.rating - 1500.0) / SCALE, self.deviation / SCALE)
	}

	fn from_glicko2(mu: f64, phi: f64, volatility: f64) -> Self {
		Self {
			rating: (mu * SCALE + 1500.0).clamp(MIN_RATING, MAX_RATING),
			deviation: (phi * SCALE).clamp(MIN_DEVIATION, MAX_DEVIATION),
			volatility: volatility.min(MAX_VOLATILITY),
		}
	}
}

impl Default for Rating {
	fn default() -> Self {
		Self {
			rating: 1500.0,
			deviation: MAX_DEVIATION,
			volatility: 0.09,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub enum Outcome {
	P1Win,
	P2Win,
	Draw,
}

/// Glicko-2 batch update for a rating period (multiple games against one or more opponents).
///
/// `games` is a list of `(opponent, score)` where score is 1.0 for win, 0.0 for loss, 0.5 for draw.
/// This is the canonical Glicko-2 usage: collect all games in a period, compute one update.
/// If `games` is empty, only the deviation widens (time passed without play).
pub fn glicko_update_batch(player: &Rating, games: &[(&Rating, f64)]) -> Rating {
	if games.is_empty() {
		let (mu, phi) = player.to_glicko2();
		let sigma = player.volatility;
		let phi_star = (phi * phi + sigma * sigma).sqrt();
		return Rating::from_glicko2(mu, phi_star, sigma);
	}

	let (mu, phi) = player.to_glicko2();
	let sigma = player.volatility;

	// Step 3+4: estimated variance v
	let v_inv: f64 = games
		.iter()
		.map(|(opp, _)| {
			let (mu_j, phi_j) = opp.to_glicko2();
			let g_j = g(phi_j);
			let e_j = e(mu, g_j, mu_j);
			g_j * g_j * e_j * (1.0 - e_j)
		})
		.sum();
	let v = 1.0 / v_inv;

	// Step 5: sum term (called delta/v in the paper, we keep it as delta_num for reuse)
	let delta_num: f64 = games
		.iter()
		.map(|(opp, score)| {
			let (mu_j, phi_j) = opp.to_glicko2();
			let g_j = g(phi_j);
			let e_j = e(mu, g_j, mu_j);
			g_j * (score - e_j)
		})
		.sum();
	let delta = v * delta_num;

	let new_sigma = new_volatility(sigma, phi, v, delta);
	let phi_star = (phi * phi + new_sigma * new_sigma).sqrt();
	let new_phi = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
	let new_mu = mu + new_phi * new_phi * delta_num;

	Rating::from_glicko2(new_mu, new_phi, new_sigma)
}

/// Glicko-2 update for a single game. Returns new ratings for both players.
///
/// Implements Glickman's algorithm (http://www.glicko.net/glicko/glicko2.pdf),
/// with Lichess's parameter choices.
pub fn glicko_update(r1: &Rating, r2: &Rating, outcome: Outcome) -> (Rating, Rating) {
	let (s1, s2) = match outcome {
		Outcome::P1Win => (1.0, 0.0),
		Outcome::P2Win => (0.0, 1.0),
		Outcome::Draw => (0.5, 0.5),
	};

	let new_r1 = update_one(r1, r2, s1);
	let new_r2 = update_one(r2, r1, s2);
	(new_r1, new_r2)
}

/// Update a single player's rating given one game result against `opponent`.
fn update_one(player: &Rating, opponent: &Rating, score: f64) -> Rating {
	let (mu, phi) = player.to_glicko2();
	let (mu_j, phi_j) = opponent.to_glicko2();
	let sigma = player.volatility;

	// Step 3: g and E
	let g_j = g(phi_j);
	let e_j = e(mu, g_j, mu_j);

	// Step 4: estimated variance (v)
	let v = 1.0 / (g_j * g_j * e_j * (1.0 - e_j));

	// Step 5: estimated improvement (delta)
	let delta = v * g_j * (score - e_j);

	// Step 5 continued: new volatility via Illinois algorithm
	let new_sigma = new_volatility(sigma, phi, v, delta);

	// Step 6: pre-rating-period phi*
	let phi_star = (phi * phi + new_sigma * new_sigma).sqrt();

	// Step 7: new phi and mu
	let new_phi = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
	let new_mu = mu + new_phi * new_phi * g_j * (score - e_j);

	Rating::from_glicko2(new_mu, new_phi, new_sigma)
}

/// Glicko-2 g function.
fn g(phi: f64) -> f64 {
	1.0 / (1.0 + 3.0 * phi * phi / (std::f64::consts::PI * std::f64::consts::PI)).sqrt()
}

/// Glicko-2 E function (expected score).
fn e(mu: f64, g_j: f64, mu_j: f64) -> f64 {
	1.0 / (1.0 + (-g_j * (mu - mu_j)).exp())
}

/// Step 5.1–5.4: compute new volatility using the Illinois variant of the
/// Brent/regula-falsi method (same as Lichess).
fn new_volatility(sigma: f64, phi: f64, v: f64, delta: f64) -> f64 {
	let a = (sigma * sigma).ln();
	let phi2 = phi * phi;
	let tau2 = TAU * TAU;

	let f = |x: f64| {
		let ex = x.exp();
		let d2 = delta * delta;
		let num1 = ex * (d2 - phi2 - v - ex);
		let denom1 = 2.0 * (phi2 + v + ex) * (phi2 + v + ex);
		let term2 = (x - a) / tau2;
		num1 / denom1 - term2
	};

	// Initial bounds
	let mut big_a = a;
	let mut big_b = if delta * delta > phi2 + v {
		(delta * delta - phi2 - v).ln()
	} else {
		let mut k = 1.0_f64;
		while f(a - k * TAU) < 0.0 {
			k += 1.0;
		}
		a - k * TAU
	};

	let mut f_a = f(big_a);
	let mut f_b = f(big_b);

	for _ in 0..MAX_ITERATIONS {
		if (big_b - big_a).abs() <= CONVERGENCE_TOLERANCE {
			break;
		}

		let big_c = big_a + (big_a - big_b) * f_a / (f_b - f_a);
		let f_c = f(big_c);

		if f_c * f_b <= 0.0 {
			big_a = big_b;
			f_a = f_b;
		} else {
			f_a /= 2.0;
		}
		big_b = big_c;
		f_b = f_c;
	}

	(big_a / 2.0).exp()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn winner_gains_loser_loses() {
		let r1 = Rating::default();
		let r2 = Rating::default();
		let (new1, new2) = glicko_update(&r1, &r2, Outcome::P1Win);
		assert!(new1.rating > 1500.0);
		assert!(new2.rating < 1500.0);
	}

	#[test]
	fn new_player_moves_more_than_established() {
		let new_player = Rating::default();
		let established = Rating {
			rating: 1500.0,
			deviation: 60.0,
			volatility: 0.06,
		};

		// Both win against a 1500-rated opponent with default RD
		let opponent = Rating::default();
		let (new_after, _) = glicko_update(&new_player, &opponent, Outcome::P1Win);
		let (est_after, _) = glicko_update(&established, &opponent, Outcome::P1Win);

		let new_delta = (new_after.rating - new_player.rating).abs();
		let est_delta = (est_after.rating - established.rating).abs();

		// New player should move significantly more
		assert!(new_delta > est_delta * 3.0, "new_delta={new_delta}, est_delta={est_delta}");
	}

	#[test]
	fn upset_gives_bigger_change() {
		let strong = Rating {
			rating: 1800.0,
			deviation: 60.0,
			volatility: 0.06,
		};
		let weak = Rating {
			rating: 1200.0,
			deviation: 60.0,
			volatility: 0.06,
		};
		// Weak player wins — upset
		let (new_strong, _) = glicko_update(&strong, &weak, Outcome::P2Win);
		let strong_loss = strong.rating - new_strong.rating;

		// Strong wins — expected
		let (new_strong2, _) = glicko_update(&strong, &weak, Outcome::P1Win);
		let strong_gain = new_strong2.rating - strong.rating;

		assert!(strong_loss > strong_gain * 3.0, "loss={strong_loss}, gain={strong_gain}");
	}

	#[test]
	fn deviation_shrinks_after_game() {
		let r1 = Rating::default();
		let r2 = Rating::default();
		let (new1, new2) = glicko_update(&r1, &r2, Outcome::P1Win);
		assert!(new1.deviation < r1.deviation);
		assert!(new2.deviation < r2.deviation);
	}

	#[test]
	fn ratings_stay_in_bounds() {
		let extreme_high = Rating {
			rating: 3900.0,
			deviation: 200.0,
			volatility: 0.09,
		};
		let extreme_low = Rating {
			rating: 500.0,
			deviation: 200.0,
			volatility: 0.09,
		};
		let (new_high, _) = glicko_update(&extreme_high, &extreme_low, Outcome::P1Win);
		let (_, new_low) = glicko_update(&extreme_high, &extreme_low, Outcome::P1Win);
		assert!(new_high.rating <= MAX_RATING);
		assert!(new_low.rating >= MIN_RATING);
		assert!(new_high.deviation >= MIN_DEVIATION);
		assert!(new_high.volatility <= MAX_VOLATILITY);
	}
}
