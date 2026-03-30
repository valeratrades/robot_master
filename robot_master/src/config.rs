use clap::{Parser, Subcommand};
use robot_master_arena::{BoardSize, config::ArenaConfig};
use v_utils::macros as v_macros;

#[derive(Parser)]
#[command(author, version, about = "Robot Master game")]
pub struct Cli {
	#[clap(flatten)]
	pub settings_flags: SettingsFlags,
	#[clap(flatten)]
	pub players: PlayerArgs,
	#[command(subcommand)]
	pub command: Commands,
}
#[derive(Clone, Debug, Parser)]
pub struct PlayerArgs {
	/// Player 1 (Cols) algorithm: manual/m, random/r, greedy/g, sadist/s
	#[arg(short = 'a', long, default_value = "manual")]
	pub player1: String,
	/// Player 2 (Rows) algorithm: manual/m, random/r, greedy/g, sadist/s
	#[arg(short = 'b', long, default_value = "random")]
	pub player2: String,
	/// Board size (5, 7, 9, or 11)
	#[arg(short = 's', long, default_value = "5")]
	pub size: BoardSize,
}
#[derive(Subcommand)]
pub enum Commands {
	/// Play the game in the terminal
	Tui,
	/// Play the game with a graphical interface
	Gui,
	//DO: `site` command that starts the leptos server
}

#[derive(Clone, Debug, Default, v_macros::LiveSettings, v_macros::MyConfigPrimitives, v_macros::Settings)]
pub struct Config {
	#[serde(default)]
	pub arena: ArenaConfig,
}
