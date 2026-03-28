use std::time::Duration;

use clap::Parser;
use robot_master::config::{Cli, Commands, LiveSettings};
use robot_master_arena::db;
use robot_master_core::game::GameConfig;

fn main() {
	let cli = Cli::parse();
	let settings = LiveSettings::new(cli.settings_flags, Duration::from_secs(5)).expect("failed to load config");
	let config = settings.config().expect("failed to read config");
	let rating_db = db::from_config(&config.arena);

	let players = cli.players;
	match cli.command {
		Commands::Tui => {
			let game_config = GameConfig::default();
			robot_master::tui::run(game_config, &players.player1, &players.player2, rating_db);
		}
		Commands::Gui => {
			let asset_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../robot_master_game/assets");
			robot_master_game::create_app(asset_dir, &players.player1, &players.player2).run();
		}
	}
}
