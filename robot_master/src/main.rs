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

	match cli.command {
		Commands::Tui { player1, player2 } => {
			let game_config = GameConfig::default();
			robot_master::tui::run(game_config, &player1, &player2, rating_db.as_ref());
		}
	}
}
