use std::collections::HashMap;

use dashmap::DashMap;
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
	pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:,
	[(); N + 1]:, {
	run_tournament(RatingBased::new(target_rounds, threads), player_ids, config, rating_db, factory, rng, threads, pb)
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
	pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:,
	[(); N + 1]:, {
	run_tournament(Swiss::new(cycles), player_ids, config, rating_db, factory, rng, threads, pb)
}
/// Round-robin tournament, repeated for `cycles` full sweeps.
///
/// Each cycle: every player plays every other player exactly once (N*(N-1)/2 games).
/// Pairings use the standard circle-method schedule so all games in one round can run in
/// parallel. Glicko-2 is updated after every game.
pub fn round_robin<const N: usize>(
	player_ids: &[Ustr],
	config: GameConfig,
	cycles: usize,
	rating_db: &dyn RatingDb,
	factory: &dyn BotFactory<N>,
	rng: &mut impl Rng,
	threads: usize,
	pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:,
	[(); N + 1]:, {
	run_tournament(RoundRobin::new(cycles), player_ids, config, rating_db, factory, rng, threads, pb)
}
/// Single-elimination tournament, repeated for `cycles` full brackets.
///
/// Each bracket:
/// 1. Sort all players by rating; pair adjacent players (rank 0 vs 1, rank 2 vs 3, …)
///    so each matchup is between players of closest ELO — under-valued players beat their
///    near-peers and advance to play more games.
/// 2. Winners advance to the next round; losers are eliminated for this bracket.
///    Draws are resolved deterministically on player id ordering so there are no byes.
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
	pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:,
	[(); N + 1]:, {
	run_tournament(Elimination::new(cycles), player_ids, config, rating_db, factory, rng, threads, pb)
}
/// Strategy for one tournament type. Implemented by `RatingBased`, `Swiss`, `Elimination`.
///
/// The shared runner [`run_tournament`] handles rating init/save, the rayon pool, and the outer
/// cycle loop. Implementors only need to provide pairing and result-application logic.
trait Tournament<const N: usize>
where
	[(); N * N]:,
	[(); N + 1]:, {
	/// Called once before the outer loop. Implementors may pre-compute constants (e.g.
	/// `rounds_per_bracket`) or seed per-run state from the initial ratings.
	fn init(&mut self, player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>);

	/// Total number of outer cycles (= progress bar ticks).
	fn cycles(&self) -> usize;

	/// Produce the list of `(p1_id, p2_id, seed)` games to play for `cycle`.
	/// Return an empty vec to skip a cycle (used by `Elimination` for padding cycles).
	fn pairs_for_cycle(&mut self, cycle: usize, player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>, rng: &mut dyn Rng) -> Vec<(Ustr, Ustr, u64)>;

	/// Called once per `MatchResult` from the current cycle, in arbitrary order.
	/// Should update `live_ratings` (Glicko-2) and any internal bookkeeping.
	fn apply_result(&mut self, result: &MatchResult, live_ratings: &DashMap<Ustr, Rating>);

	/// Called after all results for a cycle have been applied.
	/// Used for deferred batch updates (e.g. `RatingBased`) and state transitions (e.g. `Elimination`).
	fn end_cycle(&mut self, cycle: usize, player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>);
}

// ---------------------------------------------------------------------------
// Shared runner
// ---------------------------------------------------------------------------

fn run_tournament<const N: usize, T: Tournament<N>>(
	mut tourney: T,
	player_ids: &[Ustr],
	config: GameConfig,
	rating_db: &dyn RatingDb,
	factory: &dyn BotFactory<N>,
	rng: &mut impl Rng,
	threads: usize,
	mut pb: Option<&mut ProgressBar>,
) -> Vec<MatchResult>
where
	[(); N * N]:,
	[(); N + 1]:, {
	assert!(player_ids.len() >= 2, "need at least 2 players for a tournament");

	let pool = rayon::ThreadPoolBuilder::default().num_threads(threads).build().expect("failed to build rayon thread pool");

	let live_ratings: DashMap<Ustr, Rating> = {
		let loaded = rating_db.load_ratings();
		let dm: DashMap<Ustr, Rating> = loaded.into_iter().collect();
		for id in player_ids {
			dm.entry(*id).or_default();
		}
		dm
	};

	tourney.init(player_ids, &live_ratings);

	let mut all_results = Vec::default();

	for cycle in 0..tourney.cycles() {
		let pairs = tourney.pairs_for_cycle(cycle, player_ids, &live_ratings, rng as &mut dyn Rng);

		if pairs.is_empty() {
			// Padding cycle (e.g. elimination bracket already collapsed).
			tourney.end_cycle(cycle, player_ids, &live_ratings);
			if let Some(ref mut pb) = pb {
				pb.progress(cycle + 1);
			}
			continue;
		}

		let round_results: Vec<MatchResult> = if threads == 1 {
			pairs.into_iter().map(|(p1, p2, seed)| play_game::<N>(p1, p2, seed, config, factory)).collect()
		} else {
			pool.install(|| pairs.into_par_iter().map(|(p1, p2, seed)| play_game::<N>(p1, p2, seed, config, factory)).collect())
		};

		for result in &round_results {
			tourney.apply_result(result, &live_ratings);
		}
		tourney.end_cycle(cycle, player_ids, &live_ratings);

		all_results.extend(round_results);
		if let Some(ref mut pb) = pb {
			pb.progress(cycle + 1);
		}
	}

	let final_ratings: HashMap<Ustr, Rating> = live_ratings.into_iter().collect();
	rating_db.save_ratings(&final_ratings);
	all_results
}

// ---------------------------------------------------------------------------
// Rating-based
// ---------------------------------------------------------------------------

/// Each cycle picks one pair (A, B) by weighted-random + neighbor, plays `threads` games between
/// them, then does a **batch** Glicko-2 update treating all those games as one rating period.
struct RatingBased {
	target_rounds: usize,
	threads: usize,
	/// Accumulated (opponent_rating_snapshot, score) per player for the current cycle.
	/// Flushed as a single Glicko-2 batch in `end_cycle`.
	pending_a: Vec<(Rating, f64)>,
	pending_b: Vec<(Rating, f64)>,
	/// The pair chosen this cycle, so `end_cycle` knows who to update.
	current_pair: Option<(Ustr, Ustr)>,
}

impl RatingBased {
	fn new(target_rounds: usize, threads: usize) -> Self {
		Self {
			target_rounds,
			threads,
			pending_a: Vec::default(),
			pending_b: Vec::default(),
			current_pair: None,
		}
	}
}

impl<const N: usize> Tournament<N> for RatingBased
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn init(&mut self, _player_ids: &[Ustr], _live_ratings: &DashMap<Ustr, Rating>) {}

	fn cycles(&self) -> usize {
		(self.target_rounds as f64 / self.threads as f64).ceil() as usize
	}

	fn pairs_for_cycle(&mut self, cycle: usize, player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>, rng: &mut dyn Rng) -> Vec<(Ustr, Ustr, u64)> {
		let n = player_ids.len();

		// Build rating-sorted snapshot (ascending: rank 0 = weakest)
		let mut sorted: Vec<(Ustr, f64)> = player_ids.iter().map(|&id| (id, live_ratings.get(&id).map(|r| r.rating).unwrap_or(1500.0))).collect();
		sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

		// Pick A: weighted by rating
		let total: f64 = sorted.iter().map(|(_, r)| r).sum();
		let pick: f64 = rng.random::<f64>() * total;
		let mut a_rank = n - 1;
		let mut acc = 0.0;
		for (i, (_, r)) in sorted.iter().enumerate() {
			acc += r;
			if acc >= pick {
				a_rank = i;
				break;
			}
		}

		// Pick B: immediate neighbor
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
		self.current_pair = Some((a_id, b_id));

		// Snapshot ratings for the batch update (ratings may change mid-cycle for other tourneys,
		// but for rating_based each cycle is one isolated pair so we snapshot once here).
		let r_a = live_ratings.get(&a_id).map(|r| r.clone()).unwrap_or_default();
		let r_b = live_ratings.get(&b_id).map(|r| r.clone()).unwrap_or_default();
		self.pending_a.clear();
		self.pending_b.clear();
		// Store the opponent snapshot; scores filled in apply_result.
		// We need n_games slots, so reserve by stashing sentinel values that apply_result replaces.
		// Simpler: just let apply_result push; we pass the snapshot via fields.
		drop((r_a, r_b)); // will re-read in end_cycle before any update happens

		(0..self.threads)
			.map(|game_n| {
				let seed = rng.random::<u64>();
				let (p1, p2) = if (cycle + game_n) % 2 == 0 { (a_id, b_id) } else { (b_id, a_id) };
				(p1, p2, seed)
			})
			.collect()
	}

	fn apply_result(&mut self, result: &MatchResult, live_ratings: &DashMap<Ustr, Rating>) {
		let (a_id, b_id) = self.current_pair.expect("apply_result called before pairs_for_cycle");
		// Get opponent snapshot for Glicko-2 — we read current value (not yet updated this cycle).
		let r_a = live_ratings.get(&a_id).map(|r| r.clone()).unwrap_or_default();
		let r_b = live_ratings.get(&b_id).map(|r| r.clone()).unwrap_or_default();

		let (score_a, score_b) = if result.p1_id == a_id {
			(score_f64(result.p1_score, result.p2_score), score_f64(result.p2_score, result.p1_score))
		} else {
			(score_f64(result.p2_score, result.p1_score), score_f64(result.p1_score, result.p2_score))
		};
		self.pending_a.push((r_b, score_a));
		self.pending_b.push((r_a, score_b));
	}

	fn end_cycle(&mut self, _cycle: usize, _player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>) {
		let (a_id, b_id) = match self.current_pair.take() {
			Some(p) => p,
			None => return,
		};
		if self.pending_a.is_empty() {
			return;
		}

		// Read current (pre-update) ratings for the batch computation.
		let r_a = live_ratings.get(&a_id).map(|r| r.clone()).unwrap_or_default();
		let r_b = live_ratings.get(&b_id).map(|r| r.clone()).unwrap_or_default();

		// Build slices of (&Rating, score) — the opponent snapshot stored in pending is used.
		let a_games: Vec<(&Rating, f64)> = self.pending_a.iter().map(|(opp, s)| (opp as &Rating, *s)).collect();
		let b_games: Vec<(&Rating, f64)> = self.pending_b.iter().map(|(opp, s)| (opp as &Rating, *s)).collect();

		live_ratings.insert(a_id, glicko_update_batch(&r_a, &a_games));
		live_ratings.insert(b_id, glicko_update_batch(&r_b, &b_games));

		self.pending_a.clear();
		self.pending_b.clear();
	}
}

// ---------------------------------------------------------------------------
// Swiss
// ---------------------------------------------------------------------------

/// FIDE Swiss tournament. The inner "rounds per bracket" loop is **flattened** into the outer
/// cycle index: `bracket = cycle / rounds_per_bracket`, `swiss_round = cycle % rounds_per_bracket`.
struct Swiss {
	brackets: usize,
	rounds_per_bracket: usize,
	scores: HashMap<Ustr, u32>,
}

impl Swiss {
	fn new(brackets: usize) -> Self {
		Self {
			brackets,
			rounds_per_bracket: 0,
			scores: HashMap::default(),
		}
	}
}

impl<const N: usize> Tournament<N> for Swiss
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn init(&mut self, player_ids: &[Ustr], _live_ratings: &DashMap<Ustr, Rating>) {
		self.rounds_per_bracket = (player_ids.len() as f64).log2().ceil() as usize;
	}

	fn cycles(&self) -> usize {
		self.brackets * self.rounds_per_bracket
	}

	fn pairs_for_cycle(&mut self, cycle: usize, player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>, rng: &mut dyn Rng) -> Vec<(Ustr, Ustr, u64)> {
		let n = player_ids.len();
		let swiss_round = cycle % self.rounds_per_bracket;
		let bracket = cycle / self.rounds_per_bracket;

		let pairs: Vec<(usize, usize)> = if swiss_round == 0 {
			// Start of a new bracket: reset scores
			self.scores = player_ids.iter().map(|id| (*id, 0)).collect();
			// Round 1: sort by rating desc, pair rank i vs rank (n/2 + i)
			let mut order: Vec<usize> = (0..n).collect();
			order.sort_by(|&a, &b| {
				let ra = live_ratings.get(&player_ids[a]).map(|r| r.rating).unwrap_or(1500.0);
				let rb = live_ratings.get(&player_ids[b]).map(|r| r.rating).unwrap_or(1500.0);
				rb.partial_cmp(&ra).unwrap()
			});
			let n_pairs = n / 2;
			(0..n_pairs).map(|i| (order[i], order[n / 2 + i])).collect()
		} else {
			fide_pair_by_score(player_ids, &self.scores, live_ratings)
		};

		pairs
			.iter()
			.enumerate()
			.map(|(pair_idx, &(a_idx, b_idx))| {
				let (p1_idx, p2_idx) = if (bracket + swiss_round + pair_idx) % 2 == 0 { (a_idx, b_idx) } else { (b_idx, a_idx) };
				let seed = rng.random::<u64>();
				(player_ids[p1_idx], player_ids[p2_idx], seed)
			})
			.collect()
	}

	fn apply_result(&mut self, result: &MatchResult, live_ratings: &DashMap<Ustr, Rating>) {
		// Swiss score: winner gets +1
		match result.p1_score.cmp(&result.p2_score) {
			std::cmp::Ordering::Greater => *self.scores.entry(result.p1_id).or_default() += 1,
			std::cmp::Ordering::Less => *self.scores.entry(result.p2_id).or_default() += 1,
			std::cmp::Ordering::Equal => {}
		}

		glicko_update_single(result, live_ratings);
	}

	fn end_cycle(&mut self, _cycle: usize, _player_ids: &[Ustr], _live_ratings: &DashMap<Ustr, Rating>) {}
}

// ---------------------------------------------------------------------------
// Elimination
// ---------------------------------------------------------------------------

/// Single-elimination bracket, repeated for `brackets` full runs.
///
/// The inner while-loop (rounds until 1 survivor) is flattened into the outer cycle using an
/// upper-bound of `brackets * ceil(log2(n))` total cycles. Empty pairs signal a padding cycle.
struct Elimination {
	brackets: usize,
	max_inner_rounds: usize,
	n: usize,
	/// Survivors in the current bracket round.
	active: Vec<Ustr>,
	/// Winners collected so far this inner round (including bye holder).
	next_active: Vec<Ustr>,
	/// Bracket index (increments when active collapses to 1).
	current_bracket: usize,
}

impl Elimination {
	fn new(brackets: usize) -> Self {
		Self {
			brackets,
			max_inner_rounds: 0,
			n: 0,
			active: Vec::default(),
			next_active: Vec::default(),
			current_bracket: 0,
		}
	}

	fn reset_bracket(&mut self, player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>) {
		let mut v: Vec<(Ustr, f64)> = player_ids.iter().map(|&id| (id, live_ratings.get(&id).map(|r| r.rating).unwrap_or(1500.0))).collect();
		v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
		self.active = v.into_iter().map(|(id, _)| id).collect();
		self.next_active.clear();
	}

	fn resort_active(&mut self, live_ratings: &DashMap<Ustr, Rating>) {
		self.active.sort_by(|a, b| {
			let ra = live_ratings.get(a).map(|r| r.rating).unwrap_or(1500.0);
			let rb = live_ratings.get(b).map(|r| r.rating).unwrap_or(1500.0);
			ra.partial_cmp(&rb).unwrap()
		});
	}
}

impl<const N: usize> Tournament<N> for Elimination
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn init(&mut self, player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>) {
		self.n = player_ids.len();
		self.max_inner_rounds = (self.n as f64).log2().ceil() as usize;
		self.current_bracket = 0;
		self.reset_bracket(player_ids, live_ratings);
	}

	fn cycles(&self) -> usize {
		self.brackets * self.max_inner_rounds
	}

	fn pairs_for_cycle(&mut self, cycle: usize, _player_ids: &[Ustr], _live_ratings: &DashMap<Ustr, Rating>, rng: &mut dyn Rng) -> Vec<(Ustr, Ustr, u64)> {
		if self.active.len() <= 1 {
			// Bracket already collapsed — padding cycle.
			return Vec::new();
		}

		let inner_round = cycle % self.max_inner_rounds;
		self.next_active.clear();

		// Odd player gets a bye (last in sorted list).
		if self.active.len() % 2 == 1 {
			self.next_active.push(*self.active.last().unwrap());
		}

		let mut pairs = Vec::new();
		let mut i = 0;
		while i + 1 < self.active.len() {
			let seed = rng.random::<u64>();
			let (p1, p2) = if (self.current_bracket + inner_round + i) % 2 == 0 {
				(self.active[i], self.active[i + 1])
			} else {
				(self.active[i + 1], self.active[i])
			};
			pairs.push((p1, p2, seed));
			i += 2;
		}
		pairs
	}

	fn apply_result(&mut self, result: &MatchResult, live_ratings: &DashMap<Ustr, Rating>) {
		glicko_update_single(result, live_ratings);

		// Advance winner
		let winner = match result.p1_score.cmp(&result.p2_score) {
			std::cmp::Ordering::Greater => result.p1_id,
			std::cmp::Ordering::Less => result.p2_id,
			std::cmp::Ordering::Equal =>
				if result.p1_id.as_str() < result.p2_id.as_str() {
					result.p1_id
				} else {
					result.p2_id
				},
		};
		self.next_active.push(winner);
	}

	fn end_cycle(&mut self, cycle: usize, player_ids: &[Ustr], live_ratings: &DashMap<Ustr, Rating>) {
		if self.active.len() <= 1 {
			// Was a padding cycle; check if we should start the next bracket.
			let inner_round = cycle % self.max_inner_rounds;
			if inner_round == self.max_inner_rounds - 1 {
				self.current_bracket += 1;
				if self.current_bracket < self.brackets {
					self.reset_bracket(player_ids, live_ratings);
				}
			}
			return;
		}

		self.active = std::mem::take(&mut self.next_active);
		self.resort_active(live_ratings);

		// If bracket is done, start the next one on the next bracket boundary.
		if self.active.len() == 1 {
			let inner_round = cycle % self.max_inner_rounds;
			if inner_round == self.max_inner_rounds - 1 {
				// Happened to finish exactly on the last inner slot — start next bracket now.
				self.current_bracket += 1;
				if self.current_bracket < self.brackets {
					self.reset_bracket(player_ids, live_ratings);
				}
			}
			// Otherwise: remaining padding cycles will be no-ops; bracket reset happens there.
		}
	}
}

// ---------------------------------------------------------------------------
// Round Robin
// ---------------------------------------------------------------------------

/// Round-robin tournament. The outer cycle = one full sweep (every player vs every other once).
/// The circle-method schedule flattens N-1 rounds (each with floor(N/2) games) into the outer
/// cycle counter: `sched_round = cycle % (n - 1 + n % 2)`.
///
/// Because the runner's outer loop is one cycle = one progress tick, we use one cycle = one
/// full sweep. All games in a sweep are returned from `pairs_for_cycle` in a single batch.
struct RoundRobin {
	sweeps: usize,
	/// Pinned player for the circle method (index 0); the rest rotate.
	/// Length = n (or n+1 if n is odd, with the phantom "bye" slot at index n).
	schedule: Vec<Option<Ustr>>, // None = phantom bye slot
}

impl RoundRobin {
	fn new(sweeps: usize) -> Self {
		Self { sweeps, schedule: Vec::default() }
	}
}

impl<const N: usize> Tournament<N> for RoundRobin
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn init(&mut self, player_ids: &[Ustr], _live_ratings: &DashMap<Ustr, Rating>) {
		let n = player_ids.len();
		// If odd, add a phantom bye slot so the schedule math is uniform.
		if n % 2 == 0 {
			self.schedule = player_ids.iter().map(|&id| Some(id)).collect();
		} else {
			self.schedule = player_ids.iter().map(|&id| Some(id)).chain(std::iter::once(None)).collect();
		}
	}

	fn cycles(&self) -> usize {
		self.sweeps
	}

	fn pairs_for_cycle(&mut self, cycle: usize, _player_ids: &[Ustr], _live_ratings: &DashMap<Ustr, Rating>, rng: &mut dyn Rng) -> Vec<(Ustr, Ustr, u64)> {
		let n = self.schedule.len(); // even (phantom added if needed)
		let rounds = n - 1; // one full sweep needs exactly n-1 rounds

		let mut pairs = Vec::with_capacity(n / 2 * rounds);

		// Circle method: fix slot 0, rotate slots 1..n each round.
		// After `rounds` rotations we're back to the original arrangement.
		// For sweep `cycle` we rotate the schedule by `cycle * 1` positions first (optional
		// shuffle between sweeps), then enumerate all n-1 rounds within.
		//
		// We build the rotated schedule for this sweep's starting point so repeated sweeps
		// produce the same matchups in a different order (deterministic, no RNG needed for
		// pairing itself).
		let sweep_offset = cycle % rounds; // deterministic inter-sweep rotation
		let mut slots: Vec<Option<Ustr>> = self.schedule[1..].to_vec();
		slots.rotate_left(sweep_offset);
		let fixed = self.schedule[0];

		for round in 0..rounds {
			// Pair fixed vs slots[n/2 - 1], then slots[i] vs slots[n - 2 - i].
			let top = fixed;
			let bot = slots[n / 2 - 1];
			if let (Some(a), Some(b)) = (top, bot) {
				let seed = rng.random::<u64>();
				let (p1, p2) = if (cycle + round) % 2 == 0 { (a, b) } else { (b, a) };
				pairs.push((p1, p2, seed));
			}
			for i in 0..n / 2 - 1 {
				let a = slots[i];
				let b = slots[n - 2 - i];
				if let (Some(a), Some(b)) = (a, b) {
					let seed = rng.random::<u64>();
					let (p1, p2) = if (cycle + round + i + 1) % 2 == 0 { (a, b) } else { (b, a) };
					pairs.push((p1, p2, seed));
				}
			}
			// Rotate for next round.
			slots.rotate_left(1);
		}

		pairs
	}

	fn apply_result(&mut self, result: &MatchResult, live_ratings: &DashMap<Ustr, Rating>) {
		glicko_update_single(result, live_ratings);
	}

	fn end_cycle(&mut self, _cycle: usize, _player_ids: &[Ustr], _live_ratings: &DashMap<Ustr, Rating>) {}
}

/// Group by cumulative score (desc), within each group sort by rating (desc) and pair top-half vs
/// bottom-half. Odd groups float the last player into the next lower score group.
///
/// Returns pairs as `(index_into_player_ids, index_into_player_ids)`.
fn fide_pair_by_score(player_ids: &[Ustr], scores: &HashMap<Ustr, u32>, live_ratings: &DashMap<Ustr, Rating>) -> Vec<(usize, usize)> {
	let n = player_ids.len();

	let mut players: Vec<(usize, u32, f64)> = (0..n)
		.map(|i| {
			let id = player_ids[i];
			let score = scores[&id];
			let rating = live_ratings.get(&id).map(|r| r.rating).unwrap_or(1500.0);
			(i, score, rating)
		})
		.collect();

	// Sort by score desc, then rating desc
	players.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.partial_cmp(&a.2).unwrap()));

	let mut pairs = Vec::with_capacity(n / 2);
	let mut unpaired: Vec<(usize, u32, f64)> = Vec::default();
	let mut i = 0;

	while i < players.len() {
		let group_score = players[i].1;
		let mut group: Vec<(usize, u32, f64)> = unpaired.drain(..).collect();
		while i < players.len() && players[i].1 == group_score {
			group.push(players[i]);
			i += 1;
		}

		group.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

		if group.len() % 2 == 1 {
			unpaired.push(group.pop().unwrap());
		}

		let mid = group.len() / 2;
		for j in 0..mid {
			pairs.push((group[j].0, group[mid + j].0));
		}
	}

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

fn glicko_update_single(result: &MatchResult, live_ratings: &DashMap<Ustr, Rating>) {
	let r1 = live_ratings.get(&result.p1_id).map(|r| r.clone()).unwrap_or_default();
	let r2 = live_ratings.get(&result.p2_id).map(|r| r.clone()).unwrap_or_default();
	let s1 = score_f64(result.p1_score, result.p2_score);
	let s2 = score_f64(result.p2_score, result.p1_score);
	live_ratings.insert(result.p1_id, glicko_update_batch(&r1, &[(&r2, s1)]));
	live_ratings.insert(result.p2_id, glicko_update_batch(&r2, &[(&r1, s2)]));
}

fn score_f64(my_score: u16, opp_score: u16) -> f64 {
	match my_score.cmp(&opp_score) {
		std::cmp::Ordering::Greater => 1.0,
		std::cmp::Ordering::Less => 0.0,
		std::cmp::Ordering::Equal => 0.5,
	}
}
