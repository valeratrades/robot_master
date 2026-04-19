use std::{
	io::{self, BufRead, Write},
	sync::Arc,
};

use miette::Diagnostic;
use regex::Regex;
use robot_master_arena::{
	BoardSize,
	algos::{InnerKind, OnnxPlayer, PlayerKind},
	db::{NoopRatingDb, RatingDb},
	player::Bot,
	rating::Rating,
	tournament,
};
use robot_master_core::game::GameConfig;
use robot_master_train::player_kind::kind_into_bot;
use thiserror::Error;
use ustr::{Ustr, ustr};
use v_utils::io::ProgressBar;

use crate::config::{ArenaCommands, PlayersCommands, TourneyMode};

pub fn run(
	players_filter: Vec<String>,
	no_priors: Vec<String>,
	models_dir: std::path::PathBuf,
	command: ArenaCommands,
	size: BoardSize,
	hide: bool,
	rating_db: Arc<dyn RatingDb>,
	auto_yes: bool,
) {
	if !no_priors.is_empty() && !players_filter.is_empty() {
		die(miette::miette!("--no-priors and --select are mutually exclusive"));
	}
	match command {
		ArenaCommands::Tourney { mode, json } =>
			if no_priors.is_empty() {
				run_tournament(players_filter, &models_dir, mode, size, hide, rating_db, json)
			} else {
				run_tournament_no_priors(no_priors, &models_dir, mode, size, hide, json)
			},
		ArenaCommands::Players { command } => {
			if !no_priors.is_empty() {
				die(miette::miette!("--no-priors cannot be used with `players` subcommand"));
			}
			run_players(players_filter, command, &models_dir, rating_db, auto_yes)
		}
	}
}
#[derive(Debug, Diagnostic, Error)]
#[error("invalid regex pattern {pattern:?}")]
struct InvalidRegex {
	pattern: String,
	#[source]
	source: regex::Error,
}

#[derive(Debug, Diagnostic, Error)]
#[error("unknown player spec: {spec}")]
#[diagnostic(help("valid specs: random, greedy, sadist, rollout, rollout|v50, rollout|v800, rollout|g800, onnx:<stem>|g400"))]
struct UnknownPlayerSpec {
	spec: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("not enough players for a tournament: need at least 2, found {found}")]
#[diagnostic(help("add players with `arena players new`, or broaden your --select filter"))]
struct NotEnoughPlayers {
	found: usize,
}

#[derive(Debug, Diagnostic, Error)]
#[error("--select cannot be used with `players new`")]
#[diagnostic(help("omit --select to register players unconditionally"))]
struct SelectWithNew;

fn die(report: impl Into<miette::Report>) -> ! {
	eprintln!("{:?}", report.into());
	std::process::exit(1);
}

fn resolve_players(filter: &[String], models_dir: &std::path::Path, rating_db: &dyn RatingDb) -> Vec<PlayerKind> {
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
		// Auto-discover .onnx models from models_dir
		if let Ok(entries) = std::fs::read_dir(models_dir) {
			for entry in entries.flatten() {
				let path = entry.path();
				if path.extension().and_then(|e| e.to_str()) == Some("onnx")
					&& let Some(stem) = path.file_stem().and_then(|s| s.to_str())
				{
					let id = ustr(&format!("onnx:{stem}"));
					if !ids.contains(&id) {
						ids.push(id);
					}
				}
			}
		}
		ids
	};

	let matched: Vec<&Ustr> = if filter.is_empty() {
		all_ids.iter().collect()
	} else {
		let patterns: Vec<Regex> = filter
			.iter()
			.map(|pat| Regex::new(pat).unwrap_or_else(|e| die(InvalidRegex { pattern: pat.clone(), source: e })))
			.collect();
		all_ids.iter().filter(|id| patterns.iter().any(|re| re.is_match(id.as_str()))).collect()
	};

	matched
		.into_iter()
		.filter_map(|id| {
			let s = id.as_str();
			match s.parse::<PlayerKind>() {
				Ok(kind) if kind.is_manual() => None, // skip manual players for arena
				Ok(kind) if kind.is_onnx() =>
					if let InnerKind::OnnxPlayer(ref p) = kind.inner {
						let path = models_dir.join(format!("{}.onnx", p.stem));
						if path.exists() { Some(kind) } else { None }
					} else {
						panic!("is_onnx() true but inner is not OnnxPlayer")
					},
				Ok(kind) => Some(kind),
				Err(_) => None,
			}
		})
		.collect()
}

fn bot_from_kind<const N: usize>(kind: &PlayerKind, models_dir: &std::path::Path) -> Box<dyn Bot<N>>
where
	[(); N * N]:,
	[(); N + 1]:, {
	kind_into_bot(kind, models_dir).unwrap_or_else(|e| die(miette::miette!("{e}")))
}

fn run_tournament_no_priors(specs: Vec<String>, models_dir: &std::path::Path, mode: TourneyMode, size: BoardSize, hide: bool, json: bool) {
	let config = GameConfig { size: size.into(), hide };
	let kinds: Vec<PlayerKind> = specs
		.iter()
		.map(|s| s.parse::<PlayerKind>().unwrap_or_else(|_| die(UnknownPlayerSpec { spec: s.clone() })))
		.filter(|k| {
			if k.supports(&config) {
				true
			} else {
				eprintln!("Skipping {k}: not compatible with current config ({}×{}, hide={hide})", config.size, config.size);
				false
			}
		})
		.collect();
	if kinds.len() < 2 {
		die(NotEnoughPlayers { found: kinds.len() });
	}

	let (mode_label, raw_threads) = match &mode {
		TourneyMode::Swiss { cycles, threads } => (format!("Swiss ({cycles} cycles)"), *threads),
		TourneyMode::Rating { target_rounds, threads } => (format!("Rating ({target_rounds} rounds)"), *threads),
		TourneyMode::Elimination { cycles, threads } => (format!("Elimination ({cycles} cycles)"), *threads),
		TourneyMode::RoundRobin { cycles, threads } => (format!("Round-Robin ({cycles} sweeps)"), *threads),
	};
	let threads = if raw_threads == 0 {
		std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
	} else {
		raw_threads
	};

	eprintln!("{mode_label} tournament (no priors): {} players, {threads} threads", kinds.len());
	for kind in &kinds {
		eprintln!("  {kind}");
	}

	let rating_db: Arc<dyn RatingDb> = Arc::new(NoopRatingDb::default());
	match size {
		BoardSize::Five => run_tournament_sized::<5>(kinds, &std::collections::HashMap::default(), config, mode, threads, models_dir, rating_db, json),
		BoardSize::Seven => run_tournament_sized::<7>(kinds, &std::collections::HashMap::default(), config, mode, threads, models_dir, rating_db, json),
		BoardSize::Nine => run_tournament_sized::<9>(kinds, &std::collections::HashMap::default(), config, mode, threads, models_dir, rating_db, json),
		BoardSize::Eleven => run_tournament_sized::<11>(kinds, &std::collections::HashMap::default(), config, mode, threads, models_dir, rating_db, json),
	}
}

fn run_tournament(players_filter: Vec<String>, models_dir: &std::path::Path, mode: TourneyMode, size: BoardSize, hide: bool, rating_db: Arc<dyn RatingDb>, json: bool) {
	let config = GameConfig { size: size.into(), hide };
	let mut kinds = resolve_players(&players_filter, models_dir, rating_db.as_ref());
	kinds.retain(|k| {
		if k.supports(&config) {
			true
		} else {
			eprintln!("Skipping {k}: not compatible with current config ({}×{}, hide={hide})", config.size, config.size);
			false
		}
	});
	if kinds.len() < 2 {
		die(NotEnoughPlayers { found: kinds.len() });
	}

	let (mode_label, raw_threads) = match &mode {
		TourneyMode::Swiss { cycles, threads } => (format!("Swiss ({cycles} cycles)"), *threads),
		TourneyMode::Rating { target_rounds, threads } => (format!("Rating ({target_rounds} rounds)"), *threads),
		TourneyMode::Elimination { cycles, threads } => (format!("Elimination ({cycles} cycles)"), *threads),
		TourneyMode::RoundRobin { cycles, threads } => (format!("Round-Robin ({cycles} sweeps)"), *threads),
	};
	let threads = if raw_threads == 0 {
		std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
	} else {
		raw_threads
	};

	eprintln!("{mode_label} tournament: {} players, {threads} threads", kinds.len());
	for kind in &kinds {
		eprintln!("  {kind}");
	}

	let ratings_map = rating_db.load_ratings();
	let ratings_f64: std::collections::HashMap<Ustr, f64> = ratings_map.iter().map(|(k, v)| (*k, v.rating)).collect();

	match size {
		BoardSize::Five => run_tournament_sized::<5>(kinds, &ratings_f64, config, mode, threads, models_dir, rating_db, json),
		BoardSize::Seven => run_tournament_sized::<7>(kinds, &ratings_f64, config, mode, threads, models_dir, rating_db, json),
		BoardSize::Nine => run_tournament_sized::<9>(kinds, &ratings_f64, config, mode, threads, models_dir, rating_db, json),
		BoardSize::Eleven => run_tournament_sized::<11>(kinds, &ratings_f64, config, mode, threads, models_dir, rating_db, json),
	}
}

fn run_tournament_sized<const N: usize>(
	kinds: Vec<PlayerKind>,
	ratings: &std::collections::HashMap<Ustr, f64>,
	config: GameConfig,
	mode: TourneyMode,
	threads: usize,
	models_dir: &std::path::Path,
	rating_db: Arc<dyn RatingDb>,
	json: bool,
) where
	[(); N * N]:,
	[(); N + 1]:, {
	let kind_map: std::collections::HashMap<Ustr, PlayerKind> = kinds.iter().map(|k| (k.id(), k.clone())).collect();
	let player_ids: Vec<Ustr> = kinds.iter().map(|k| k.id()).collect();
	let models_dir = models_dir.to_path_buf();

	let factory = move |id: Ustr| -> Box<dyn Bot<N>> {
		let kind = &kind_map[&id];
		bot_from_kind::<N>(kind, &models_dir)
	};

	let mut rng = rand::make_rng::<rand::rngs::SmallRng>();

	let (pb_label, estimated_total) = match mode {
		TourneyMode::Swiss { cycles, .. } => ("Swiss", cycles),
		TourneyMode::Rating { target_rounds, .. } => {
			let cycles = (target_rounds as f64 / threads as f64).ceil() as usize;
			("Rating", cycles)
		}
		TourneyMode::Elimination { cycles, .. } => ("Elimination", cycles),
		TourneyMode::RoundRobin { cycles, .. } => ("RoundRobin", cycles),
	};
	let mut pb = ProgressBar::builder().total(estimated_total).prefix(pb_label.to_string()).build();
	pb.init();

	let (results, final_ratings) = match mode {
		TourneyMode::Swiss { cycles, .. } => tournament::swiss::<N>(&player_ids, config, cycles, rating_db.as_ref(), &factory, &mut rng, threads, Some(&mut pb)),
		TourneyMode::Rating { target_rounds, .. } => tournament::rating_based::<N>(&player_ids, config, target_rounds, rating_db.as_ref(), &factory, &mut rng, threads, Some(&mut pb)),
		TourneyMode::Elimination { cycles, .. } => tournament::elimination::<N>(&player_ids, config, cycles, rating_db.as_ref(), &factory, &mut rng, threads, Some(&mut pb)),
		TourneyMode::RoundRobin { cycles, .. } => tournament::round_robin::<N>(&player_ids, config, cycles, rating_db.as_ref(), &factory, &mut rng, threads, Some(&mut pb)),
	};
	pb.finish();

	// Print summary
	let mut wins: std::collections::HashMap<Ustr, u32> = std::collections::HashMap::default();
	let mut games: std::collections::HashMap<Ustr, u32> = std::collections::HashMap::default();
	for r in &results {
		*games.entry(r.p1_id).or_default() += 1;
		*games.entry(r.p2_id).or_default() += 1;
		match r.p1_score.cmp(&r.p2_score) {
			std::cmp::Ordering::Greater => *wins.entry(r.p1_id).or_default() += 1,
			std::cmp::Ordering::Less => *wins.entry(r.p2_id).or_default() += 1,
			std::cmp::Ordering::Equal => {}
		}
	}

	let mut standings: Vec<_> = final_ratings.iter().filter(|(id, _)| games.contains_key(id)).collect();
	standings.sort_by(|a, b| {
		let wa = wins.get(a.0).copied().unwrap_or(0);
		let wb = wins.get(b.0).copied().unwrap_or(0);
		wb.cmp(&wa).then_with(|| b.1.rating.partial_cmp(&a.1.rating).unwrap())
	});

	let rows: Vec<(String, String, String, String)> = standings
		.iter()
		.map(|(id, rating)| {
			let w = wins.get(*id).copied().unwrap_or(0);
			let g = games.get(*id).copied().unwrap_or(0);
			let prov = if rating.is_provisional() { "?" } else { "" };
			let prev = ratings.get(*id).copied().unwrap_or(rating.rating);
			let delta = rating.rating - prev;
			let sign = if delta >= 0.0 { "+" } else { "" };
			(
				format!("{w}/{g}"),
				id.to_string(),
				format!("{:.0}{prov}", rating.rating),
				format!("{sign}{delta:.0}, RD {:.0}", rating.deviation),
			)
		})
		.collect();

	if json {
		let entries: Vec<String> = standings
			.iter()
			.map(|(id, _)| {
				let w = wins.get(*id).copied().unwrap_or(0);
				let g = games.get(*id).copied().unwrap_or(0);
				format!(r#"{{"id":"{id}","wins":{w},"games":{g}}}"#)
			})
			.collect();
		println!("[{}]", entries.join(","));
	} else {
		let col0 = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
		let col1 = rows.iter().map(|r| r.1.len()).max().unwrap_or(0);
		let col2 = rows.iter().map(|r| r.2.len()).max().unwrap_or(0);

		eprintln!("\n--- Results ({} games) ---", results.len());
		for (wins_col, name, rating_col, delta_col) in &rows {
			eprintln!("  {wins_col:<col0$}  {name:<col1$}  {rating_col:>col2$}  ({delta_col})");
		}
	}

	rating_db.save_ratings(&final_ratings);
}

fn run_players(players_filter: Vec<String>, command: PlayersCommands, models_dir: &std::path::Path, rating_db: Arc<dyn RatingDb>, auto_yes: bool) {
	let mut ratings = rating_db.load_ratings();

	let matched: Vec<Ustr> = if players_filter.is_empty() {
		ratings.keys().copied().collect()
	} else {
		let patterns: Vec<Regex> = players_filter
			.iter()
			.map(|pat| Regex::new(pat).unwrap_or_else(|e| die(InvalidRegex { pattern: pat.clone(), source: e })))
			.collect();
		ratings.keys().filter(|id| patterns.iter().any(|re| re.is_match(id.as_str()))).copied().collect()
	};

	if let PlayersCommands::New { .. } = command
		&& !players_filter.is_empty()
	{
		die(SelectWithNew);
	}
	if let PlayersCommands::New {
		players,
		sizes,
		hide: hide_constraint,
	} = command
	{
		let constrain_sizes: Option<Vec<u8>> = if sizes.is_empty() { None } else { Some(sizes) };
		let mut explicit: Vec<PlayerKind> = Vec::default();
		for spec in &players {
			let mut kind: PlayerKind = spec.parse().unwrap_or_else(|_| die(UnknownPlayerSpec { spec: spec.clone() }));
			if kind.is_manual() {
				die(miette::miette!("cannot register manual players in arena - manual players participate via the TUI/GUI only"));
			}
			if kind.is_onnx() {
				kind.constrain_sizes = constrain_sizes.clone();
				kind.constrain_hide = hide_constraint;
			} else if constrain_sizes.is_some() || hide_constraint.is_some() {
				eprintln!("Note: --sizes/--hide ignored for rule-based bot {spec} (rule-based bots support all regimes)");
			}
			explicit.push(kind);
		}

		// Also auto-register any missing default variants
		let mut to_register: Vec<PlayerKind> = explicit;
		for kind in PlayerKind::defaults() {
			if !to_register.iter().any(|k| k.id() == kind.id()) {
				to_register.push(kind);
			}
		}
		// Auto-register all .onnx models present in models_dir
		if let Ok(entries) = std::fs::read_dir(models_dir) {
			for entry in entries.flatten() {
				let path = entry.path();
				if path.extension().and_then(|e| e.to_str()) == Some("onnx")
					&& let Some(stem) = path.file_stem().and_then(|s| s.to_str())
				{
					let kind = PlayerKind {
						inner: InnerKind::OnnxPlayer(OnnxPlayer { stem: stem.to_string() }),
						sims: None,
						constrain_sizes: None,
						constrain_hide: None,
					};
					if !to_register.iter().any(|k| k.id() == kind.id()) {
						to_register.push(kind);
					}
				}
			}
		}

		let mut registered = 0usize;
		for kind in &to_register {
			let id = kind.id();
			if ratings.contains_key(&id) {
				continue;
			}
			ratings.insert(id, Rating::default());
			eprintln!("Registered {id} (rating {:.0}, RD {:.0})", Rating::default().rating, Rating::default().deviation);
			registered += 1;
		}

		if registered > 0 {
			rating_db.save_ratings(&ratings);
		} else {
			eprintln!("All players already registered.");
		}
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
		let mut answer = String::default();
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
