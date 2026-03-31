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
	Gui {
		/// Enable music and sound effects
		#[arg(long, default_value = "false")]
		sound: bool,
	},
	/// Arena: tournaments and data management
	Arena {
		/// Filter players by grepping these patterns against known IDs. If empty, all players.
		#[arg(short, long, value_delimiter = ',')]
		select: Vec<String>,
		#[command(subcommand)]
		command: ArenaCommands,
	},
	//DO: `site` command that starts the leptos server
}

#[derive(Subcommand)]
pub enum ArenaCommands {
	/// Run a tournament
	Tourney {
		#[command(subcommand)]
		mode: TourneyMode,
	},
	/// Player data management
	Players {
		#[command(subcommand)]
		command: PlayersCommands,
	},
}

#[derive(Subcommand)]
pub enum TourneyMode {
	/// True FIDE Swiss: 1 game/pairing, pair within score groups, runs N full brackets
	Swiss {
		/// Number of full Swiss brackets to run
		#[arg(default_value = "10")]
		cycles: usize,
		/// Number of threads (0 = all cores)
		#[arg(short, long, default_value = "0")]
		threads: usize,
	},
	/// Rating-based: weighted-random pairing by ELO proximity, ceil(target_rounds / threads) cycles
	Rating {
		/// Total games to play (split across cycles of `threads` games each)
		#[arg(default_value = "100")]
		target_rounds: usize,
		/// Number of threads (0 = all cores)
		#[arg(short, long, default_value = "0")]
		threads: usize,
	},
	/// Single-elimination: pair by ELO proximity, winners advance, repeat for N cycles
	Elimination {
		/// Number of full elimination brackets to run
		#[arg(default_value = "10")]
		cycles: usize,
		/// Number of threads (0 = all cores)
		#[arg(short, long, default_value = "0")]
		threads: usize,
	},
}

#[derive(Subcommand)]
pub enum PlayersCommands {
	/// Register player algorithms (e.g. `mcts:s500`, `random`, `rollout`). Also auto-registers any missing default variants.
	New {
		/// Player specs: algo names with optional params (e.g. `mcts:s500`, `greedy`)
		players: Vec<String>,
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
