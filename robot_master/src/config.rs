use clap::{Parser, Subcommand};
use robot_master_arena::{BoardSize, config::ArenaConfig};
use v_utils::macros as v_macros;

#[derive(Parser)]
#[command(author, version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")"), about, long_about = None)]
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
	/// Arena: tournaments and data management
	Arena {
		/// Filter players by grepping these patterns against known IDs. If empty, all players.
		#[arg(trailing_var_arg = true)]
		players: Vec<String>,
		#[command(subcommand)]
		command: ArenaCommands,
	},
	//DO: `site` command that starts the leptos server
}

#[derive(Subcommand)]
pub enum ArenaCommands {
	/// Run a Swiss tournament
	Tourney {
		/// Average number of games per pairing
		#[arg(default_value = "1")]
		rounds: usize,
	},
	/// Player data management
	Players {
		#[command(subcommand)]
		command: PlayersCommands,
	},
}

#[derive(Subcommand)]
pub enum PlayersCommands {
	/// Register a new player algorithm (e.g. `mcts:s500`, `random`, `rollout`)
	New {
		/// Player spec: algo name with optional params (e.g. `mcts:s500`, `greedy`)
		player: String,
	},
	/// List all players and their ratings
	List,
	/// Reset ratings to default (all if no players filter, otherwise only matched)
	ResetRatings,
	/// Remove players from the database entirely
	Nuke,
}

#[derive(Clone, Debug, Default, v_macros::LiveSettings, v_macros::MyConfigPrimitives, v_macros::Settings)]
pub struct Config {
	#[serde(default)]
	pub arena: ArenaConfig,
}
