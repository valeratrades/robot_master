use std::{sync::Arc, time::Duration};

use clap::Parser;
pub mod config;
fn main() {
	v_utils::clientside!();
	let cli = Cli::parse();
	let live_settings = match LiveSettings::new(cli.settings, Duration::from_secs(5)) {
		Ok(ls) => Arc::new(ls),
		Err(e) => {
			eprintln!("Error reading config: {e}");
			for cause in e.chain().skip(1) {
				eprintln!("  Caused by: {cause}");
			}
			return;
		}
	};
	run(live_settings);
}
mod sprite_animation;
use config::{LiveSettings, SettingsFlags};

#[derive(Default, Parser)]
#[command(author, version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")"), about, long_about = None)]
struct Cli {
	#[command(flatten)]
	settings: SettingsFlags,
}

fn run(settings: Arc<LiveSettings>) {
	sprite_animation::run();
}
