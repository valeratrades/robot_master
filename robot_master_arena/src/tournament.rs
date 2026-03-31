use std::{
	collections::HashMap,
	sync::atomic::{AtomicUsize, Ordering},
};

use rand::{Rng, RngExt, SeedableRng, rngs::SmallRng};
use rayon::prelude::*;
use robot_master_core::game::{GameConfig, GameState};
use ustr::Ustr;
use v_utils::io::ProgressBar;

use crate::{
	db::RatingDb,
	match_::{Match, MatchResult},
	player::Bot,
};

/// Something that can produce a fresh `Bot<N>` for a given player id.
/// Must be `Send + Sync` so rayon threads can call it concurrently.
pub trait BotFactory<const N: usize>: Send + Sync
where
	[(); N * N]:, {
	fn create(&self, id: Ustr) -> Box<dyn Bot<N>>;
}

/// Blanket impl for closures.
impl<const N: usize, F> BotFactory<N> for F
where
	F: Fn(Ustr) -> Box<dyn Bot<N>> + Send + Sync,
	[(); N * N]:,
{
	fn create(&self, id: Ustr) -> Box<dyn Bot<N>> {
		(self)(id)
	}
}

/// Swiss-system tournament with log-weighted round allocation.
///
/// Each Swiss round:
/// 1. Sort players by (tournament wins descending, then rating descending as tiebreak)
/// 2. Pair adjacent players
/// 3. Allocate games per pairing: `max(1, round(avg_rounds * ln(2 + combined_score) / mean_weight))`
/// 4. Play the allocated games in parallel across pairings, update scores
///
/// Number of Swiss rounds = ceil(log2(n_players)).
pub fn swiss<const N: usize>(
	player_ids: &[Ustr],
	ratings: &HashMap<Ustr, f64>,
	config: GameConfig,
	avg_rounds: usize,
	rating_db: &dyn RatingDb,
	factory: &dyn BotFactory<N>,
	rng: &mut impl Rng,
	threads: usize,
	mut pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:, {
	let n = player_ids.len();
	assert!(n >= 2, "need at least 2 players for a tournament");

	let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().expect("failed to build rayon thread pool");

	let swiss_rounds = (n as f64).log2().ceil() as usize;
	let mut scores: HashMap<Ustr, u32> = player_ids.iter().map(|id| (*id, 0)).collect();
	let mut all_results = Vec::new();
	let progress = AtomicUsize::new(0);

	for swiss_round in 0..swiss_rounds {
		// Sort player indices by (score desc, rating desc)
		let mut order: Vec<usize> = (0..n).collect();
		order.sort_by(|&a, &b| {
			let sa = scores[&player_ids[a]];
			let sb = scores[&player_ids[b]];
			sb.cmp(&sa).then_with(|| {
				let ra = ratings.get(&player_ids[a]).copied().unwrap_or(1500.0);
				let rb = ratings.get(&player_ids[b]).copied().unwrap_or(1500.0);
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
				let combined = scores[&player_ids[a]] + scores[&player_ids[b]];
				(2.0 + combined as f64).ln()
			})
			.collect();
		let mean_weight = raw_weights.iter().sum::<f64>() / raw_weights.len() as f64;

		// Build work items: (pair_idx, game_n, p1_id, p2_id, rng_seed)
		let mut work: Vec<(Ustr, Ustr, u64)> = Vec::new();
		for (pair_idx, &(a_idx, b_idx)) in pairs.iter().enumerate() {
			let pair_rounds = (avg_rounds as f64 * raw_weights[pair_idx] / mean_weight).round().max(1.0) as usize;
			for game_n in 0..pair_rounds {
				let (p1_idx, p2_idx) = if (swiss_round + game_n) % 2 == 0 { (a_idx, b_idx) } else { (b_idx, a_idx) };
				let seed = rng.random::<u64>();
				work.push((player_ids[p1_idx], player_ids[p2_idx], seed));
			}
		}

		// Run all games for this round in parallel
		let round_results: Vec<MatchResult> = if threads == 1 {
			work.iter()
				.map(|&(p1_id, p2_id, seed)| {
					let result = play_game::<N>(p1_id, p2_id, seed, config, factory);
					progress.fetch_add(1, Ordering::Relaxed);
					if let Some(ref mut pb) = pb {
						pb.progress(progress.load(Ordering::Relaxed));
					}
					result
				})
				.collect()
		} else {
			pool.install(|| {
				work.par_iter()
					.map(|&(p1_id, p2_id, seed)| {
						let result = play_game::<N>(p1_id, p2_id, seed, config, factory);
						progress.fetch_add(1, Ordering::Relaxed);
						result
					})
					.collect()
			})
		};

		// Sequential: update ratings and scores
		for mut result in round_results {
			result.update_rating(rating_db);

			match result.p1_score.cmp(&result.p2_score) {
				std::cmp::Ordering::Greater => *scores.entry(result.p1_id).or_default() += 1,
				std::cmp::Ordering::Less => *scores.entry(result.p2_id).or_default() += 1,
				std::cmp::Ordering::Equal => {}
			}

			all_results.push(result);
			if let Some(ref mut pb) = pb {
				pb.progress(all_results.len());
			}
		}
	}

	all_results
}

/// Run a round-robin tournament: every player plays every other player `rounds` times,
/// alternating who goes first.
pub fn round_robin<const N: usize>(player_ids: &[Ustr], config: GameConfig, rounds: usize, factory: &dyn BotFactory<N>, rng: &mut impl Rng) -> Vec<MatchResult>
where
	[(); N * N]:, {
	let n = player_ids.len();
	let mut results = Vec::new();

	for round in 0..rounds {
		for i in 0..n {
			for j in (i + 1)..n {
				let (p1_idx, p2_idx) = if round % 2 == 0 { (i, j) } else { (j, i) };
				let seed = rng.random::<u64>();
				let result = play_game::<N>(player_ids[p1_idx], player_ids[p2_idx], seed, config, factory);
				results.push(result);
			}
		}
	}

	results
}
fn play_game<const N: usize>(p1_id: Ustr, p2_id: Ustr, seed: u64, config: GameConfig, factory: &dyn BotFactory<N>) -> MatchResult
where
	[(); N * N]:, {
	let mut rng = SmallRng::seed_from_u64(seed);
	let game = GameState::<N>::new(config, &mut rng);
	let p1 = factory.create(p1_id);
	let p2 = factory.create(p2_id);
	Match::new(game, p1, p2, p1_id, p2_id).run()
}
