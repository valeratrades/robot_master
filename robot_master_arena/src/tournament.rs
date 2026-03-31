use std::collections::HashMap;

use rand::Rng;
use robot_master_core::game::{GameConfig, GameState};
use ustr::Ustr;

use crate::{
	db::RatingDb,
	match_::{Match, MatchResult},
	player::Bot,
};

/// Run a round-robin tournament: every player plays every other player `rounds` times,
/// alternating who goes first.
///
/// Uses `dyn Player` — this is the orchestration layer, not the hot path.
/// For self-play training (millions of games), use `Match` directly with concrete types.
pub fn round_robin<const N: usize>(players: &mut [Box<dyn Bot<N>>], config: GameConfig, rounds: usize, rng: &mut impl Rng) -> Vec<MatchResult>
where
	[(); N * N]:, {
	let n = players.len();
	let mut results = Vec::new();

	for round in 0..rounds {
		for i in 0..n {
			for j in (i + 1)..n {
				// Alternate who plays first each round
				let (p1_idx, p2_idx) = if round % 2 == 0 { (i, j) } else { (j, i) };

				let game = GameState::<N>::new(config, rng);

				// We need to borrow two players mutably at once.
				// Safe because p1_idx != p2_idx.
				assert!(p1_idx != p2_idx);
				let (p1, p2) = if p1_idx < p2_idx {
					let (left, right) = players.split_at_mut(p2_idx);
					(&mut *left[p1_idx], &mut *right[0])
				} else {
					let (left, right) = players.split_at_mut(p1_idx);
					(&mut *right[0], &mut *left[p2_idx])
				};

				let result = run_dyn_match::<N>(game, p1, p2);
				results.push(result);
			}
		}
	}

	results
}

/// Swiss-system tournament with log-weighted round allocation.
///
/// Each Swiss round:
/// 1. Sort players by (tournament wins descending, then rating descending as tiebreak)
/// 2. Pair adjacent players
/// 3. Allocate games per pairing: `max(1, round(avg_rounds * ln(2 + combined_score) / mean_weight))`
/// 4. Play the allocated games, update scores
///
/// Number of Swiss rounds = ceil(log2(n_players)).
pub fn swiss<const N: usize>(
	players: &mut [Box<dyn Bot<N>>],
	ratings: &HashMap<Ustr, f64>,
	config: GameConfig,
	avg_rounds: usize,
	rating_db: &dyn RatingDb,
	rng: &mut impl Rng,
) -> Vec<MatchResult>
where
	[(); N * N]:, {
	let n = players.len();
	assert!(n >= 2, "need at least 2 players for a tournament");

	let swiss_rounds = (n as f64).log2().ceil() as usize;
	// tournament score: wins per player
	let mut scores: HashMap<Ustr, u32> = players.iter().map(|p| (p.id(), 0)).collect();
	let mut all_results = Vec::new();

	for swiss_round in 0..swiss_rounds {
		// Sort player indices by (score desc, rating desc)
		let mut order: Vec<usize> = (0..n).collect();
		order.sort_by(|&a, &b| {
			let sa = scores[&players[a].id()];
			let sb = scores[&players[b].id()];
			sb.cmp(&sa).then_with(|| {
				let ra = ratings.get(&players[a].id()).copied().unwrap_or(1500.0);
				let rb = ratings.get(&players[b].id()).copied().unwrap_or(1500.0);
				rb.partial_cmp(&ra).unwrap()
			})
		});

		// Pair adjacent
		let n_pairs = n / 2;
		let pairs: Vec<(usize, usize)> = (0..n_pairs).map(|i| (order[2 * i], order[2 * i + 1])).collect();

		// Compute log-weighted round allocation
		let raw_weights: Vec<f64> = pairs
			.iter()
			.map(|&(a, b)| {
				let combined = scores[&players[a].id()] + scores[&players[b].id()];
				(2.0 + combined as f64).ln()
			})
			.collect();
		let mean_weight = raw_weights.iter().sum::<f64>() / raw_weights.len() as f64;

		for (pair_idx, &(a_idx, b_idx)) in pairs.iter().enumerate() {
			let pair_rounds = (avg_rounds as f64 * raw_weights[pair_idx] / mean_weight).round().max(1.0) as usize;

			for game_n in 0..pair_rounds {
				// Alternate who goes first
				let (p1_idx, p2_idx) = if (swiss_round + game_n) % 2 == 0 { (a_idx, b_idx) } else { (b_idx, a_idx) };

				let game = GameState::<N>::new(config, rng);

				assert!(p1_idx != p2_idx);
				let (p1, p2) = if p1_idx < p2_idx {
					let (left, right) = players.split_at_mut(p2_idx);
					(&mut *left[p1_idx], &mut *right[0])
				} else {
					let (left, right) = players.split_at_mut(p1_idx);
					(&mut *right[0], &mut *left[p2_idx])
				};

				let mut result = run_dyn_match::<N>(game, p1, p2);
				result.update_rating(rating_db);

				// Update tournament scores
				match result.p1_score.cmp(&result.p2_score) {
					std::cmp::Ordering::Greater => *scores.entry(result.p1_id).or_default() += 1,
					std::cmp::Ordering::Less => *scores.entry(result.p2_id).or_default() += 1,
					std::cmp::Ordering::Equal => {} // draw: no points
				}

				all_results.push(result);
			}
		}
	}

	all_results
}

fn run_dyn_match<const N: usize>(game: GameState<N>, p1: &mut dyn Bot<N>, p2: &mut dyn Bot<N>) -> MatchResult
where
	[(); N * N]:, {
	Match::new(game, p1, p2).run()
}
