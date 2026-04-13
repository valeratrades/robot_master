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
	/// Hide opponent's hand (information-hidden mode)
	#[arg(long, default_value = "false")]
	pub hide: bool,
	/// Directory containing .onnx model files
	#[arg(long, default_value = "./models")]
	pub models_dir: std::path::PathBuf,
}
#[derive(Clone, Debug, Parser)]
pub struct TrainArgs {
	/// Include the git hash in the run ID (fully pins to exact build; fragments cache across commits)
	#[arg(long)]
	pub exact_generation: bool,
	/// Number of selfplay → train → export iterations
	#[arg(long, default_value = "20")]
	pub iterations: u32,
	/// Self-play games per iteration
	#[arg(long, default_value = "200")]
	pub games: u32,
	/// Gumbel simulations per move during self-play
	#[arg(long, default_value = "25")]
	pub sims: u32,
	/// Board size (must match the selfplay binary and model architecture)
	#[arg(long, default_value = "5")]
	pub size: u32,
	/// Pass --force-cpu to selfplay (sequential rayon, faster at 5×5/7×7).
	#[arg(long)]
	pub force_cpu: bool,
	/// Hide opponent's hand (information-hidden mode).
	#[arg(long)]
	pub hide: bool,
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
		/// Run an ephemeral tournament with these player specs (e.g. `rollout|v50 rollout|g200`).
		/// Bypasses the ratings database entirely — no prior ratings loaded, nothing saved.
		/// Mutually exclusive with --select.
		#[arg(long, value_delimiter = ',')]
		no_priors: Vec<String>,
		#[command(subcommand)]
		command: ArenaCommands,
	},
	/// Train a neural network model (AlphaZero selfplay → train → export loop)
	Train {
		#[command(subcommand)]
		arch: TrainArch,
	},
	//DO: `site` command that starts the leptos server
}

#[derive(Subcommand)]
pub enum TrainArch {
	/// ResNet CNN architecture
	Cnn {
		#[clap(flatten)]
		args: TrainArgs,
		/// Supervised pre-training spec (e.g. `rollout|v50`). Controls both:
		/// selfplay data generation (uses this bot until NN beats it >68% over 32 games),
		/// and eval matches run every 10 versions to detect the threshold.
		/// If omitted, starts NN selfplay immediately with no eval.
		#[arg(long)]
		supervise: Option<String>,
	},
	/// Transformer architecture
	Transformer {
		#[clap(flatten)]
		args: TrainArgs,
	},
}

#[derive(Subcommand)]
pub enum ArenaCommands {
	/// Run a tournament
	Tourney {
		#[command(subcommand)]
		mode: TourneyMode,
		/// Output results as JSON to stdout (progress and status remain on stderr).
		#[arg(long)]
		json: bool,
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
	/// Round-robin: every player plays every other exactly once per sweep, repeat for N sweeps
	RoundRobin {
		/// Number of full round-robin sweeps to run
		#[arg(default_value = "3")]
		cycles: usize,
		/// Number of threads (0 = all cores)
		#[arg(short, long, default_value = "0")]
		threads: usize,
	},
}

#[derive(Subcommand)]
pub enum PlayersCommands {
	/// Register player algorithms (e.g. `rollout|800`, `random`, `onnx:model_v5|g200|s5|hh`). Also auto-registers any missing default variants.
	New {
		/// Player specs: algo names with optional sim counts (e.g. `rollout|800`, `greedy`, `onnx:model_v5|g400`)
		players: Vec<String>,
		/// Constrain to specific board sizes, e.g. `5,7`. Required for onnx bots; ignored for rule-based bots.
		#[arg(long, value_delimiter = ',')]
		sizes: Vec<u8>,
		/// Constrain to a specific hide mode. Optional; omit to support both modes.
		#[arg(long)]
		hide: Option<bool>,
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
