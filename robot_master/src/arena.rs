use std::io::{self, BufRead, Write};

use robot_master_arena::{
	BoardSize,
	algos::{PlayerKind, rollout::Rollout},
	db::RatingDb,
	player::Bot,
	rating::Rating,
	tournament,
};
use robot_master_core::game::GameConfig;
use robot_master_train::mcts::{MctsBot, MctsConfig, RolloutEval};
use ustr::Ustr;
use v_utils::io::ProgressBar;

use crate::config::{ArenaCommands, PlayersCommands};

pub fn run(players_filter: Vec<String>, command: ArenaCommands, size: BoardSize, rating_db: Box<dyn RatingDb>, auto_yes: bool) {
	match command {
		ArenaCommands::Tourney { rounds, threads } => run_tournament(players_filter, rounds, threads, size, rating_db),
		ArenaCommands::Players { command } => run_players(players_filter, command, rating_db, auto_yes),
	}
}

fn resolve_players(filter: &[String], rating_db: &dyn RatingDb) -> Vec<PlayerKind> {
	let all_ids: Vec<Ustr> = {
		let ratings = rating_db.load_ratings();
		let mut ids: Vec<Ustr> = ratings.keys().copied().collect();
		// Also include all known algo defaults even if they don't have ratings yet
		for kind in PlayerKind::defaults() {
			let id = kind.id();
			if !ids.contains(&id) {
				ids.push(id);
			}
		}
		ids
	};

	let matched: Vec<&Ustr> = if filter.is_empty() {
		all_ids.iter().collect()
	} else {
		all_ids.iter().filter(|id| filter.iter().any(|pat| id.as_str().contains(pat.as_str()))).collect()
	};

	matched
		.into_iter()
		.filter_map(|id| {
			let s = id.as_str();
			match s.parse::<PlayerKind>() {
				Ok(kind) if kind.is_manual() => None, // skip manual players for arena
				Ok(kind) => Some(kind),
				Err(_) => None,
			}
		})
		.collect()
}

fn kind_into_bot<const N: usize>(kind: &PlayerKind) -> Box<dyn Bot<N>>
where
	[(); N * N]:, {
	match kind {
		PlayerKind::Mcts(params) => {
			let evaluator = RolloutEval::new(Rollout {});
			let config = MctsConfig {
				simulations: params.simulations,
				c_puct: 1.41,
			};
			Box::new(MctsBot::new(evaluator, config))
		}
		other => other.clone().into_bot(),
	}
}

fn run_tournament(players_filter: Vec<String>, avg_rounds: usize, threads: usize, size: BoardSize, rating_db: Box<dyn RatingDb>) {
	let kinds = resolve_players(&players_filter, rating_db.as_ref());
	if kinds.len() < 2 {
		eprintln!("Need at least 2 players for a tournament, found {}", kinds.len());
		std::process::exit(1);
	}

	let threads = if threads == 0 {
		std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
	} else {
		threads
	};

	eprintln!("Swiss tournament: {} players, ~{avg_rounds} avg rounds/pairing, {threads} threads", kinds.len());
	for kind in &kinds {
		eprintln!("  {kind}");
	}

	let config = GameConfig {
		size: size.into(),
		..GameConfig::default()
	};

	// Load initial ratings for seeding
	let ratings_map = rating_db.load_ratings();
	let ratings_f64: std::collections::HashMap<Ustr, f64> = ratings_map.iter().map(|(k, v)| (*k, v.rating)).collect();

	match size {
		BoardSize::Five => run_tournament_sized::<5>(kinds, &ratings_f64, config, avg_rounds, threads, rating_db),
		BoardSize::Seven => run_tournament_sized::<7>(kinds, &ratings_f64, config, avg_rounds, threads, rating_db),
		BoardSize::Nine => run_tournament_sized::<9>(kinds, &ratings_f64, config, avg_rounds, threads, rating_db),
		BoardSize::Eleven => run_tournament_sized::<11>(kinds, &ratings_f64, config, avg_rounds, threads, rating_db),
	}
}

fn run_tournament_sized<const N: usize>(
	kinds: Vec<PlayerKind>,
	ratings: &std::collections::HashMap<Ustr, f64>,
	config: GameConfig,
	avg_rounds: usize,
	threads: usize,
	rating_db: Box<dyn RatingDb>,
) where
	[(); N * N]:, {
	// Build id→kind lookup for the factory
	let kind_map: std::collections::HashMap<Ustr, PlayerKind> = kinds.iter().map(|k| (k.id(), k.clone())).collect();
	let player_ids: Vec<Ustr> = kinds.iter().map(|k| k.id()).collect();

	let factory = move |id: Ustr| -> Box<dyn Bot<N>> {
		let kind = &kind_map[&id];
		kind_into_bot::<N>(kind)
	};

	let mut rng = rand::make_rng::<rand::rngs::SmallRng>();

	let swiss_rounds = (player_ids.len() as f64).log2().ceil() as usize;
	let estimated_total = (player_ids.len() / 2) * avg_rounds * swiss_rounds;
	let mut pb = ProgressBar::builder().total(estimated_total).prefix("Swiss".into()).build();

	let results = tournament::swiss::<N>(&player_ids, ratings, config, avg_rounds, rating_db.as_ref(), &factory, &mut rng, threads, Some(&mut pb));

	// Print summary
	let mut wins: std::collections::HashMap<Ustr, u32> = std::collections::HashMap::new();
	let mut games: std::collections::HashMap<Ustr, u32> = std::collections::HashMap::new();
	for r in &results {
		*games.entry(r.p1_id).or_default() += 1;
		*games.entry(r.p2_id).or_default() += 1;
		match r.p1_score.cmp(&r.p2_score) {
			std::cmp::Ordering::Greater => *wins.entry(r.p1_id).or_default() += 1,
			std::cmp::Ordering::Less => *wins.entry(r.p2_id).or_default() += 1,
			std::cmp::Ordering::Equal => {}
		}
	}

	// Final ratings
	let final_ratings = rating_db.load_ratings();
	let mut standings: Vec<_> = final_ratings.iter().filter(|(id, _)| games.contains_key(id)).collect();
	standings.sort_by(|a, b| b.1.rating.partial_cmp(&a.1.rating).unwrap());

	eprintln!("\n--- Results ({} games) ---", results.len());
	for (id, rating) in &standings {
		let w = wins.get(id).copied().unwrap_or(0);
		let g = games.get(id).copied().unwrap_or(0);
		let prov = if rating.is_provisional() { "?" } else { "" };
		let prev = ratings.get(id).copied().unwrap_or(rating.rating);
		let delta = rating.rating - prev;
		let sign = if delta >= 0.0 { "+" } else { "" };
		eprintln!("  {id}: {:.0}{prov} ({sign}{delta:.0}, RD {:.0})  {w}/{g} wins", rating.rating, rating.deviation);
	}
}

fn run_players(players_filter: Vec<String>, command: PlayersCommands, rating_db: Box<dyn RatingDb>, auto_yes: bool) {
	let mut ratings = rating_db.load_ratings();

	let matched: Vec<Ustr> = if players_filter.is_empty() {
		ratings.keys().copied().collect()
	} else {
		ratings.keys().filter(|id| players_filter.iter().any(|pat| id.as_str().contains(pat.as_str()))).copied().collect()
	};

	if let PlayersCommands::New { player } = command {
		let kind: PlayerKind = player.parse().unwrap_or_else(|e| {
			eprintln!("Unknown player spec: {e}");
			std::process::exit(1);
		});
		if kind.is_manual() {
			eprintln!("Cannot register manual players in arena");
			std::process::exit(1);
		}
		let id = kind.id();
		if ratings.contains_key(&id) {
			eprintln!("Player already exists: {id}");
			std::process::exit(1);
		}
		ratings.insert(id, Rating::default());
		rating_db.save_ratings(&ratings);
		eprintln!("Registered {id} (rating {:.0}, RD {:.0})", Rating::default().rating, Rating::default().deviation);
		return;
	}

	if matches!(command, PlayersCommands::List) {
		let mut entries: Vec<_> = ratings.iter().filter(|(id, _)| matched.contains(id)).collect();
		entries.sort_by(|a, b| b.1.rating.partial_cmp(&a.1.rating).unwrap());
		if entries.is_empty() {
			eprintln!("No players found.");
		}
		for (id, r) in &entries {
			let prov = if r.is_provisional() { "?" } else { "" };
			eprintln!("  {id}: {:.0}{prov} (RD {:.0}, vol {:.4})", r.rating, r.deviation, r.volatility);
		}
		return;
	}

	if matched.is_empty() {
		eprintln!("No matching players found.");
		return;
	}

	let nuke = matches!(command, PlayersCommands::Nuke);
	let action = if nuke { "nuke" } else { "reset ratings for" };

	if matched.len() >= 10 && !auto_yes {
		eprintln!("About to {action} {} players:", matched.len());
		for id in &matched {
			eprintln!("  {id}");
		}
		eprint!("Confirm? [y/N] ");
		io::stderr().flush().unwrap();
		let mut answer = String::new();
		io::stdin().lock().read_line(&mut answer).unwrap();
		if !answer.trim().eq_ignore_ascii_case("y") {
			eprintln!("Aborted.");
			return;
		}
	}

	for id in &matched {
		if nuke {
			ratings.remove(id);
		} else {
			ratings.insert(*id, Default::default());
		}
	}
	rating_db.save_ratings(&ratings);

	let verb = if nuke { "Nuked" } else { "Reset" };
	eprintln!("{verb} {} players:", matched.len());
	for id in &matched {
		eprintln!("  {id}");
	}
}
