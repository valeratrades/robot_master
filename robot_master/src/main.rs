use std::{io::Write, process, str::FromStr, sync::Arc, time::Duration};

use clap::Parser;
use robot_master::config::{Cli, Commands, LiveSettings};
use robot_master_arena::{
	algos::{PlayerKind, validate_manual_name},
	db,
};
use robot_master_core::game::GameConfig;

fn main() {
	let cli = Cli::parse();
	let auto_yes = cli.settings_flags.yes;
	let settings = LiveSettings::new(cli.settings_flags, Duration::from_secs(5)).expect("failed to load config");
	let config = settings.config().expect("failed to read config");
	let rating_db: Arc<dyn robot_master_arena::db::RatingDb> = Arc::from(db::from_config(&config.arena));

	let size = cli.players.size;
	let models_dir = cli.players.models_dir.clone();

	match cli.command {
		Commands::Tui => {
			let p1 = resolve_player(&cli.players.player1, auto_yes);
			let p2 = resolve_player(&cli.players.player2, auto_yes);
			let game_config = GameConfig { size: size.into() };
			robot_master::tui::run(game_config, size, p1, p2, rating_db, models_dir);
		}
		Commands::Gui { sound } => {
			let p1 = resolve_player(&cli.players.player1, auto_yes);
			let p2 = resolve_player(&cli.players.player2, auto_yes);
			let asset_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../robot_master_game/assets");
			robot_master_game::create_app(asset_dir, size, p1, p2, sound, models_dir).run();
		}
		Commands::Arena { select, command } => {
			robot_master::arena::run(select, models_dir, command, size, rating_db, auto_yes);
		}
	}
}

/// Resolve a CLI player argument to a `PlayerKind`.
///
/// On exact match (case-insensitive, with shortcuts): returns immediately.
/// Otherwise: asks whether to register a manual player with that name,
/// or falls back to fzf selection over all known algo names.
fn resolve_player(input: &str, auto_yes: bool) -> PlayerKind {
	if let Ok(kind) = PlayerKind::from_str(input) {
		return kind;
	}

	validate_manual_name(input).unwrap_or_else(|e| {
		eprintln!("{e}");
		process::exit(1);
	});

	if auto_yes {
		return PlayerKind {
			inner: robot_master_arena::algos::InnerKind::ManualPlayer(robot_master_arena::player::ManualPlayer { name: input.to_string() }),
			sims: None,
		};
	}

	eprint!("Unknown player \"{input}\". Register as manual player? [y/N] ");
	std::io::stderr().flush().unwrap();
	let mut answer = String::new();
	std::io::stdin().read_line(&mut answer).unwrap();
	if answer.trim().eq_ignore_ascii_case("y") {
		return PlayerKind {
			inner: robot_master_arena::algos::InnerKind::ManualPlayer(robot_master_arena::player::ManualPlayer { name: input.to_string() }),
			sims: None,
		};
	}

	// Fall back to fzf selection over algo names
	let algo_names: Vec<String> = PlayerKind::defaults().into_iter().map(|k| k.to_string()).collect();
	let all_names: Vec<&str> = std::iter::once("manual").chain(algo_names.iter().map(|s| s.as_str())).collect();
	let fzf_input = all_names.join("\n");

	let mut child = process::Command::new("fzf")
		.arg("--query")
		.arg(input)
		.arg("--select-1")
		.arg("--header")
		.arg("Select a player algorithm:")
		.stdin(process::Stdio::piped())
		.stdout(process::Stdio::piped())
		.stderr(process::Stdio::inherit())
		.spawn()
		.unwrap_or_else(|e| {
			eprintln!("failed to launch fzf: {e}");
			process::exit(1);
		});

	child.stdin.take().unwrap().write_all(fzf_input.as_bytes()).unwrap();
	let output = child.wait_with_output().unwrap();

	if !output.status.success() {
		eprintln!("fzf cancelled");
		process::exit(1);
	}

	let selected = String::from_utf8(output.stdout).unwrap();
	let selected = selected.trim();
	PlayerKind::from_str(selected).unwrap_or_else(|_| panic!("fzf returned unrecognized player: {selected:?}"))
}
