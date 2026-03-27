use clap::{Parser, Subcommand};
use robot_master_core::game::GameConfig;

#[derive(Parser)]
#[command(author, version, about = "Robot Master game")]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Play the game in the terminal
	Tui {
		/// Player 1 plays manually (otherwise random AI)
		#[arg(short = 'm', long)]
		manual: bool,
	},
	//DO: `site` command that starts the leptos server
}

fn main() {
	let cli = Cli::parse();

	match cli.command {
		Commands::Tui { manual } => {
			let config = GameConfig::default();
			let manual_flags = [manual, false];
			let names = ["Alice", "Bob"];
			robot_master::tui::run(config, manual_flags, names);
		}
	}
}
