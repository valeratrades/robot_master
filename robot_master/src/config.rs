use clap::{Parser, Subcommand};
use robot_master_arena::config::ArenaConfig;
use v_utils::macros as v_macros;

#[derive(Parser)]
#[command(author, version, about = "Robot Master game")]
pub struct Cli {
	#[clap(flatten)]
	pub settings_flags: SettingsFlags,
	#[command(subcommand)]
	pub command: Commands,
}
#[derive(Subcommand)]
pub enum Commands {
	/// Play the game in the terminal
	Tui {
		/// Player 1 (Cols) algorithm: manual/m, random/r, greedy/g, sadist/s
		#[arg(short = 'a', long, default_value = "manual")]
		player1: String,
		/// Player 2 (Rows) algorithm: manual/m, random/r, greedy/g, sadist/s
		#[arg(short = 'b', long, default_value = "random")]
		player2: String,
	},
	//DO: `site` command that starts the leptos server
}
#[derive(Clone, Debug, Default, v_macros::LiveSettings, v_macros::MyConfigPrimitives, v_macros::Settings)]
pub struct Config {
	#[serde(default)]
	pub arena: ArenaConfig,
}
