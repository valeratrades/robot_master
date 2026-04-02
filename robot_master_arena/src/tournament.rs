use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use rand::{Rng, RngExt, SeedableRng, rngs::SmallRng};
use rayon::prelude::*;
use robot_master_core::game::{GameConfig, GameState, Player, PlayerSigned};
use ustr::Ustr;
use v_utils::io::ProgressBar;

use crate::{
	db::RatingDb,
	match_::{Match, MatchResult},
	player::Bot,
	rating::{Rating, glicko_update_batch},
};

/// Something that can produce a fresh `Bot<N>` for a given player id.
/// Must be `Send + Sync` so rayon threads can call it concurrently.
pub trait BotFactory<const N: usize>: Send + Sync
where
	[(); N * N]:,
	[(); N + 1]:, {
	fn create(&self, id: Ustr) -> Box<dyn Bot<N>>;
}

/// Blanket impl for closures.
impl<const N: usize, F> BotFactory<N> for F
where
	F: Fn(Ustr) -> Box<dyn Bot<N>> + Send + Sync,
	[(); N * N]:,
	[(); N + 1]:,
{
	fn create(&self, id: Ustr) -> Box<dyn Bot<N>> {
		(self)(id)
	}
}

/// Rating-based tournament.
///
/// Each cycle:
/// 1. Pick player A by weighted random: probability proportional to their rating
/// 2. Sort all players by current rating; pick player B = the neighbor of A (one rank up or down,
///    coin-flip; top player always goes down, bottom always goes up)
/// 3. Play `threads` games between A and B in parallel (batch Glicko-2 update after)
///
/// Total cycles = `ceil(target_rounds / threads)`.
pub fn rating_based<const N: usize>(
	player_ids: &[Ustr],
	config: GameConfig,
	target_rounds: usize,
	rating_db: &dyn RatingDb,
	factory: &dyn BotFactory<N>,
	rng: &mut impl Rng,
	threads: usize,
	mut pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:,
	[(); N + 1]:, {
	let n = player_ids.len();
	assert!(n >= 2, "need at least 2 players for a tournament");

	let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().expect("failed to build rayon thread pool");

	let live_ratings: Arc<Mutex<HashMap<Ustr, Rating>>> = {
		let mut map = rating_db.load_ratings();
		for id in player_ids {
			map.entry(*id).or_default();
		}
		Arc::new(Mutex::new(map))
	};

	let cycles = (target_rounds as f64 / threads as f64).ceil() as usize;
	let mut all_results = Vec::new();

	for cycle in 0..cycles {
		// Build rating-sorted snapshot (ascending so rank 0 = weakest)
		let sorted: Vec<(Ustr, f64)> = {
			let map = live_ratings.lock().unwrap();
			let mut v: Vec<(Ustr, f64)> = player_ids.iter().map(|&id| (id, map.get(&id).map(|r| r.rating).unwrap_or(1500.0))).collect();
			v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
			v
		};

		// Pick player A: weighted by rating (higher rating = more likely)
		let total_rating: f64 = sorted.iter().map(|(_, r)| r).sum();
		let pick: f64 = rng.random::<f64>() * total_rating;
		let mut a_rank = n - 1;
		let mut acc = 0.0;
		for (i, (_, r)) in sorted.iter().enumerate() {
			acc += r;
			if acc >= pick {
				a_rank = i;
				break;
			}
		}

		// Pick player B: immediate neighbor; top goes down, bottom goes up, others coin-flip
		let b_rank = if a_rank == 0 {
			1
		} else if a_rank == n - 1 {
			n - 2
		} else if rng.random::<bool>() {
			a_rank + 1
		} else {
			a_rank - 1
		};

		let a_id = sorted[a_rank].0;
		let b_id = sorted[b_rank].0;

		// Play `threads` games between this pair in parallel
		let seeds: Vec<u64> = (0..threads).map(|_| rng.random::<u64>()).collect();

		let pair_results: Vec<MatchResult> = if threads == 1 {
			seeds
				.into_iter()
				.enumerate()
				.map(|(game_n, seed)| {
					let (p1_id, p2_id) = if (cycle + game_n) % 2 == 0 { (a_id, b_id) } else { (b_id, a_id) };
					play_game::<N>(p1_id, p2_id, seed, config, factory)
				})
				.collect()
		} else {
			pool.install(|| {
				seeds
					.into_par_iter()
					.enumerate()
					.map(|(game_n, seed)| {
						let (p1_id, p2_id) = if (cycle + game_n) % 2 == 0 { (a_id, b_id) } else { (b_id, a_id) };
						play_game::<N>(p1_id, p2_id, seed, config, factory)
					})
					.collect()
			})
		};

		// Batch Glicko-2 update for this pairing
		{
			let mut map = live_ratings.lock().unwrap();
			let r_a = map.entry(a_id).or_default().clone();
			let r_b = map.entry(b_id).or_default().clone();

			let a_games: Vec<(&Rating, f64)> = pair_results
				.iter()
				.map(|res| {
					let score = if res.p1_id == a_id {
						score_f64(res.p1_score, res.p2_score)
					} else {
						score_f64(res.p2_score, res.p1_score)
					};
					(&r_b as &Rating, score)
				})
				.collect();
			let b_games: Vec<(&Rating, f64)> = pair_results
				.iter()
				.map(|res| {
					let score = if res.p1_id == b_id {
						score_f64(res.p1_score, res.p2_score)
					} else {
						score_f64(res.p2_score, res.p1_score)
					};
					(&r_a as &Rating, score)
				})
				.collect();

			let new_r_a = glicko_update_batch(&r_a, &a_games);
			let new_r_b = glicko_update_batch(&r_b, &b_games);
			map.insert(a_id, new_r_a);
			map.insert(b_id, new_r_b);
		}

		all_results.extend(pair_results);
		if let Some(ref mut pb) = pb {
			pb.progress(cycle + 1);
		}
	}

	rating_db.save_ratings(&live_ratings.lock().unwrap());
	all_results
}

/// True FIDE Swiss tournament.
///
/// Each "bracket" (one full Swiss sweep):
/// - Round 1: sort by rating desc, pair rank i vs rank (n/2 + i)
/// - Subsequent rounds: group by cumulative score desc, within each group sort by rating desc
///   and pair top-half vs bottom-half; odd-sized groups float the last player down to the next group
/// - Exactly 1 game per pairing per bracket round
/// - Scores and ratings update after each round
///
/// `cycles` full brackets are run; win counts accumulate across all of them.
pub fn swiss<const N: usize>(
	player_ids: &[Ustr],
	config: GameConfig,
	cycles: usize,
	rating_db: &dyn RatingDb,
	factory: &dyn BotFactory<N>,
	rng: &mut impl Rng,
	threads: usize,
	mut pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:,
	[(); N + 1]:, {
	let n = player_ids.len();
	assert!(n >= 2, "need at least 2 players for a tournament");

	let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().expect("failed to build rayon thread pool");

	let live_ratings: Arc<Mutex<HashMap<Ustr, Rating>>> = {
		let mut map = rating_db.load_ratings();
		for id in player_ids {
			map.entry(*id).or_default();
		}
		Arc::new(Mutex::new(map))
	};

	let rounds_per_bracket = (n as f64).log2().ceil() as usize;
	let mut all_results = Vec::new();

	for bracket in 0..cycles {
		// Reset scores for this bracket
		let mut scores: HashMap<Ustr, u32> = player_ids.iter().map(|id| (*id, 0)).collect();

		for swiss_round in 0..rounds_per_bracket {
			let pairs: Vec<(usize, usize)> = if swiss_round == 0 {
				// Round 1: sort by rating desc, pair rank i vs rank (n/2 + i)
				let mut order: Vec<usize> = (0..n).collect();
				{
					let map = live_ratings.lock().unwrap();
					order.sort_by(|&a, &b| {
						let ra = map.get(&player_ids[a]).map(|r| r.rating).unwrap_or(1500.0);
						let rb = map.get(&player_ids[b]).map(|r| r.rating).unwrap_or(1500.0);
						rb.partial_cmp(&ra).unwrap()
					});
				}
				let n_pairs = n / 2;
				(0..n_pairs).map(|i| (order[i], order[n / 2 + i])).collect()
			} else {
				// Subsequent rounds: group by score, within group sort by rating, pair top vs bottom half
				fide_pair_by_score(player_ids, &scores, &live_ratings)
			};

			// Build work: one game per pair
			let pair_work: Vec<(usize, Ustr, Ustr, u64)> = pairs
				.iter()
				.enumerate()
				.map(|(pair_idx, &(a_idx, b_idx))| {
					let (p1_idx, p2_idx) = if (bracket + swiss_round) % 2 == 0 { (a_idx, b_idx) } else { (b_idx, a_idx) };
					let seed = rng.random::<u64>();
					(pair_idx, player_ids[p1_idx], player_ids[p2_idx], seed)
				})
				.collect();

			let round_results: Vec<(usize, MatchResult)> = if threads == 1 {
				pair_work
					.into_iter()
					.map(|(pair_idx, p1_id, p2_id, seed)| (pair_idx, play_game::<N>(p1_id, p2_id, seed, config, factory)))
					.collect()
			} else {
				pool.install(|| {
					pair_work
						.into_par_iter()
						.map(|(pair_idx, p1_id, p2_id, seed)| (pair_idx, play_game::<N>(p1_id, p2_id, seed, config, factory)))
						.collect()
				})
			};

			// Update ratings and scores; collect results
			for (pair_idx, result) in round_results {
				let (a_idx, b_idx) = pairs[pair_idx];
				let p1_id = player_ids[a_idx];
				let p2_id = player_ids[b_idx];

				// Determine the canonical p1/p2 for this pairing (may be swapped vs played order)
				// We track score for the pairing's "a" and "b" player
				let (canon_a_score, canon_b_score) = if result.p1_id == player_ids[a_idx] {
					(result.p1_score, result.p2_score)
				} else {
					(result.p2_score, result.p1_score)
				};

				match canon_a_score.cmp(&canon_b_score) {
					std::cmp::Ordering::Greater => *scores.entry(player_ids[a_idx]).or_default() += 1,
					std::cmp::Ordering::Less => *scores.entry(player_ids[b_idx]).or_default() += 1,
					std::cmp::Ordering::Equal => {}
				}

				// Single-game Glicko-2 update
				{
					let mut map = live_ratings.lock().unwrap();
					let r1 = map.entry(p1_id).or_default().clone();
					let r2 = map.entry(p2_id).or_default().clone();

					let (s1, s2) = if result.p1_id == p1_id {
						(score_f64(result.p1_score, result.p2_score), score_f64(result.p2_score, result.p1_score))
					} else {
						(score_f64(result.p2_score, result.p1_score), score_f64(result.p1_score, result.p2_score))
					};
					let new_r1 = glicko_update_batch(&r1, &[(&r2, s1)]);
					let new_r2 = glicko_update_batch(&r2, &[(&r1, s2)]);
					map.insert(p1_id, new_r1);
					map.insert(p2_id, new_r2);
				}

				all_results.push(result);
			}
		}
		if let Some(ref mut pb) = pb {
			pb.progress(bracket + 1);
		}
	}

	rating_db.save_ratings(&live_ratings.lock().unwrap());
	all_results
}

/// Single-elimination tournament, repeated for `cycles` full brackets.
///
/// Each bracket:
/// 1. Sort all players by rating; pair adjacent players (rank 0 vs 1, rank 2 vs 3, …)
///    so each matchup is between players of closest ELO — under-valued players beat their
///    near-peers and advance to play more games.
/// 2. Winners advance to the next round; losers are eliminated for this bracket.
///    Draws are resolved by coin-flip (seeded from the game seed) so there are no byes.
/// 3. Glicko-2 is updated after every game. The bracket collapses until one player remains.
/// 4. Repeat from step 1 for the next cycle.
///
/// A player with an odd draw in any round receives a bye (no game, advances automatically).
pub fn elimination<const N: usize>(
	player_ids: &[Ustr],
	config: GameConfig,
	cycles: usize,
	rating_db: &dyn RatingDb,
	factory: &dyn BotFactory<N>,
	rng: &mut impl Rng,
	threads: usize,
	mut pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:,
	[(); N + 1]:, {
	let n = player_ids.len();
	assert!(n >= 2, "need at least 2 players for a tournament");

	let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().expect("failed to build rayon thread pool");

	let live_ratings: Arc<Mutex<HashMap<Ustr, Rating>>> = {
		let mut map = rating_db.load_ratings();
		for id in player_ids {
			map.entry(*id).or_default();
		}
		Arc::new(Mutex::new(map))
	};

	let mut all_results = Vec::new();

	for cycle in 0..cycles {
		// Sort all players by rating asc so adjacent = closest ELO
		let mut active: Vec<Ustr> = {
			let map = live_ratings.lock().unwrap();
			let mut v: Vec<(Ustr, f64)> = player_ids.iter().map(|&id| (id, map.get(&id).map(|r| r.rating).unwrap_or(1500.0))).collect();
			v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
			v.into_iter().map(|(id, _)| id).collect()
		};

		while active.len() > 1 {
			// Build pairs: adjacent players. Odd one out gets a bye (appended to next round as-is).
			let mut pairs: Vec<(Ustr, Ustr, u64)> = Vec::new();
			let mut bye: Option<Ustr> = None;

			let mut i = 0;
			while i + 1 < active.len() {
				let seed = rng.random::<u64>();
				let (p1, p2) = if (cycle + i) % 2 == 0 { (active[i], active[i + 1]) } else { (active[i + 1], active[i]) };
				pairs.push((p1, p2, seed));
				i += 2;
			}
			if active.len() % 2 == 1 {
				bye = Some(*active.last().unwrap());
			}

			// Play all games in this elimination round in parallel
			let round_results: Vec<MatchResult> = if threads == 1 {
				pairs.into_iter().map(|(p1_id, p2_id, seed)| play_game::<N>(p1_id, p2_id, seed, config, factory)).collect()
			} else {
				pool.install(|| pairs.into_par_iter().map(|(p1_id, p2_id, seed)| play_game::<N>(p1_id, p2_id, seed, config, factory)).collect())
			};

			// Determine winners, update ratings, collect results
			let mut next_active: Vec<Ustr> = Vec::new();
			if let Some(b) = bye {
				next_active.push(b);
			}

			for result in round_results {
				let (winner, loser) = match result.p1_score.cmp(&result.p2_score) {
					std::cmp::Ordering::Greater => (result.p1_id, result.p2_id),
					std::cmp::Ordering::Less => (result.p2_id, result.p1_id),
					// Draw: use the game seed's low bit via a deterministic tiebreak on ids
					std::cmp::Ordering::Equal =>
						if result.p1_id.as_str() < result.p2_id.as_str() {
							(result.p1_id, result.p2_id)
						} else {
							(result.p2_id, result.p1_id)
						},
				};

				// Glicko-2 update
				{
					let mut map = live_ratings.lock().unwrap();
					let r1 = map.entry(result.p1_id).or_default().clone();
					let r2 = map.entry(result.p2_id).or_default().clone();
					let (s1, s2) = (score_f64(result.p1_score, result.p2_score), score_f64(result.p2_score, result.p1_score));
					let new_r1 = glicko_update_batch(&r1, &[(&r2, s1)]);
					let new_r2 = glicko_update_batch(&r2, &[(&r1, s2)]);
					map.insert(result.p1_id, new_r1);
					map.insert(result.p2_id, new_r2);
				}

				next_active.push(winner);
				let _ = loser; // eliminated for this bracket
				all_results.push(result);
			}

			// Re-sort survivors by current rating so next round also pairs by proximity
			{
				let map = live_ratings.lock().unwrap();
				next_active.sort_by(|a, b| {
					let ra = map.get(a).map(|r| r.rating).unwrap_or(1500.0);
					let rb = map.get(b).map(|r| r.rating).unwrap_or(1500.0);
					ra.partial_cmp(&rb).unwrap()
				});
			}
			active = next_active;
		}
		if let Some(ref mut pb) = pb {
			pb.progress(cycle + 1);
		}
	}

	rating_db.save_ratings(&live_ratings.lock().unwrap());
	all_results
}

/// FIDE Swiss pairing for rounds 2+: group by cumulative score (desc), within each group sort by
/// rating (desc) and pair top-half vs bottom-half. Odd groups float the last player into the next
/// lower score group.
///
/// Returns pairs as `(index_into_player_ids, index_into_player_ids)`.
fn fide_pair_by_score(player_ids: &[Ustr], scores: &HashMap<Ustr, u32>, live_ratings: &Arc<Mutex<HashMap<Ustr, Rating>>>) -> Vec<(usize, usize)> {
	let n = player_ids.len();

	// Build (player_index, score, rating) triples
	let map = live_ratings.lock().unwrap();
	let mut players: Vec<(usize, u32, f64)> = (0..n)
		.map(|i| {
			let id = player_ids[i];
			let score = scores[&id];
			let rating = map.get(&id).map(|r| r.rating).unwrap_or(1500.0);
			(i, score, rating)
		})
		.collect();
	drop(map);

	// Sort by score desc, then rating desc
	players.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.partial_cmp(&a.2).unwrap()));

	let mut pairs = Vec::with_capacity(n / 2);
	let mut unpaired: Vec<(usize, u32, f64)> = Vec::new();
	let mut i = 0;

	while i < players.len() {
		// Collect the current score group (including any floaters from previous group)
		let group_score = players[i].1;
		let mut group: Vec<(usize, u32, f64)> = unpaired.drain(..).collect();
		while i < players.len() && players[i].1 == group_score {
			group.push(players[i]);
			i += 1;
		}

		// Sort group by rating desc
		group.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

		// If odd, float the last player to the next group
		if group.len() % 2 == 1 {
			let floater = group.pop().unwrap();
			unpaired.push(floater);
		}

		// Pair top-half vs bottom-half
		let mid = group.len() / 2;
		for j in 0..mid {
			pairs.push((group[j].0, group[mid + j].0));
		}
	}

	// Any remaining unpaired floaters (can only happen if n is odd overall — bye)
	// With an even n this should never leave anyone unpaired.
	debug_assert!(unpaired.is_empty() || n % 2 == 1, "unpaired players remain with even n");

	pairs
}

fn play_game<const N: usize>(p1_id: Ustr, p2_id: Ustr, seed: u64, config: GameConfig, factory: &dyn BotFactory<N>) -> MatchResult
where
	[(); N * N]:,
	[(); N + 1]:, {
	let mut rng = SmallRng::seed_from_u64(seed);
	let game = GameState::<N>::new(config, &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
	let p1 = factory.create(p1_id);
	let p2 = factory.create(p2_id);
	Match::new(game, p1, p2, p1_id, p2_id).run()
}

fn score_f64(my_score: u16, opp_score: u16) -> f64 {
	match my_score.cmp(&opp_score) {
		std::cmp::Ordering::Greater => 1.0,
		std::cmp::Ordering::Less => 0.0,
		std::cmp::Ordering::Equal => 0.5,
	}
}
