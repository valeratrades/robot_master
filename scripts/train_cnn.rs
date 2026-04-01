#!/usr/bin/env -S cargo -Zscript -q
---
[dependencies]
clap = { version = "4", features = ["derive"] }
indicatif = "0.17"
---

use std::{
	env, fs,
	path::{Path, PathBuf},
	process::{Command, Stdio},
	time::Instant,
};

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, Parser)]
#[command(about = "AlphaZero CNN training loop: selfplay → train → export → repeat")]
struct Args {
	/// Generation name — all data/checkpoints/models are scoped under this label (e.g. "v1", "cnn_big")
	generation: String,
	/// Number of selfplay → train → export iterations
	#[arg(long, default_value = "20")]
	iterations: u32,
	/// Self-play games per iteration
	#[arg(long, default_value = "200")]
	games: u32,
	/// MCTS simulations per move during self-play
	#[arg(long, default_value = "25")]
	sims: u32,
	/// Training epochs per iteration
	#[arg(long, default_value = "5")]
	epochs: u32,
}

fn main() {
	let args = Args::parse();

	let data_dir = xdg_cache_dir(&format!("{}/training_data", args.generation));
	let models_dir = xdg_cache_dir(&format!("{}/models", args.generation));

	// Detect the zlib path once (needed for Python/numpy on NixOS)
	let zlib_path = zlib_ld_path();
	let repo_root = repo_root();

	let selfplay_bin = repo_root.join("target/debug/selfplay");
	let train_py = repo_root.join("training/train.py");
	let export_py = repo_root.join("training/export_onnx.py");

	// Find the highest existing model version to resume from
	let mut version = latest_model_version(&models_dir);
	let current_model: Option<PathBuf> = if version > 0 {
		let p = models_dir.join(format!("model_v{version}.onnx"));
		if p.exists() {
			eprintln!("Resuming from model_v{version}.onnx");
			Some(p)
		} else {
			None
		}
	} else {
		None
	};
	let mut current_model = current_model;

	// Build selfplay binary upfront
	eprintln!("Building selfplay binary...");
	run_or_die(
		Command::new("cargo").args(["b", "-p", "robot_master_train", "--bin", "selfplay"]).current_dir(&repo_root),
		"cargo build",
	);

	let total_start = Instant::now();

	let bar = ProgressBar::new(args.iterations as u64);
	bar.set_style(ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} iterations  elapsed {elapsed_precise}  eta {eta_precise}").unwrap());

	for i in 1..=args.iterations {
		let iter_start = Instant::now();
		eprintln!("\n━━━ Iteration {i}/{} ━━━", args.iterations);

		// 1. Self-play
		eprint!("  [1/3] Self-play ({} games, {} sims)... ", args.games, args.sims);
		let mut selfplay_cmd = Command::new(&selfplay_bin);
		selfplay_cmd
			.args(["--games", &args.games.to_string()])
			.args(["--sims", &args.sims.to_string()])
			.args(["--output", data_dir.to_str().unwrap()])
			.current_dir(&repo_root);
		if let Some(ref model) = current_model {
			selfplay_cmd.args(["--model".as_ref(), model.to_str().unwrap()]);
		}
		let sp_start = Instant::now();
		run_or_die(&mut selfplay_cmd, "selfplay");
		eprintln!("done ({:.1}s)", sp_start.elapsed().as_secs_f64());

		// 2. Train
		eprint!("  [2/3] Training ({} epochs)... ", args.epochs);
		let train_start = Instant::now();
		let output = retry_on_clock_error(|| {
			Command::new("python")
				.args([train_py.to_str().unwrap()])
				.args(["--data-dir", data_dir.to_str().unwrap()])
				.args(["--output-dir", models_dir.to_str().unwrap()])
				.args(["--epochs", &args.epochs.to_string()])
				.env("LD_LIBRARY_PATH", &zlib_path)
				.current_dir(&repo_root)
				.output()
				.expect("failed to run train.py")
		});
		if !output.status.success() {
			eprintln!("FAILED");
			eprintln!("{}", String::from_utf8_lossy(&output.stderr));
			std::process::exit(1);
		}
		// Extract last loss line from stdout
		let stdout = String::from_utf8_lossy(&output.stdout);
		let last_epoch = stdout.lines().filter(|l| l.starts_with("Epoch")).last().unwrap_or("(no output)");
		eprintln!("done ({:.1}s)  {last_epoch}", train_start.elapsed().as_secs_f64());

		// 3. Export ONNX
		version += 1;
		let onnx_path = models_dir.join(format!("model_v{version}.onnx"));
		// Find the latest checkpoint written by train.py
		let checkpoint = latest_checkpoint(&models_dir).expect("no checkpoint found after training");
		eprint!("  [3/3] Exporting {} → model_v{version}.onnx... ", checkpoint.file_name().unwrap().to_str().unwrap());
		let export_start = Instant::now();
		let export_out = retry_on_clock_error(|| {
			Command::new("python")
				.args([export_py.to_str().unwrap()])
				.args(["--checkpoint", checkpoint.to_str().unwrap()])
				.args(["--output", onnx_path.to_str().unwrap()])
				.env("LD_LIBRARY_PATH", &zlib_path)
				.current_dir(&repo_root)
				.output()
				.expect("failed to run export_onnx.py")
		});
		if !export_out.status.success() {
			eprintln!("FAILED");
			eprintln!("{}", String::from_utf8_lossy(&export_out.stderr));
			std::process::exit(1);
		}
		eprintln!("done ({:.1}s)", export_start.elapsed().as_secs_f64());

		current_model = Some(onnx_path);

		eprintln!(
			"  iteration {i} complete in {:.1}s  (total elapsed: {:.0}s)",
			iter_start.elapsed().as_secs_f64(),
			total_start.elapsed().as_secs_f64(),
		);
		bar.inc(1);
	}

	bar.finish_and_clear();
	eprintln!("\nDone. Final model: {}", current_model.unwrap().display());
	eprintln!("To run in the arena:  robot_master arena --models-dir {} tourney rating 200", models_dir.display());
}

/// PyTorch can fail at CUDA init with a non-monotonic clock assertion — a transient OS timing
/// glitch. Retry up to 3 times since a fresh process usually succeeds.
fn retry_on_clock_error(mut f: impl FnMut() -> std::process::Output) -> std::process::Output {
	const CLOCK_ERR: &str = "getCount is non-monotonic";
	for attempt in 1..=3 {
		let out = f();
		if out.status.success() || !String::from_utf8_lossy(&out.stderr).contains(CLOCK_ERR) {
			return out;
		}
		eprintln!("\n  [retry {attempt}/3] PyTorch clock assertion — retrying...");
	}
	f()
}

fn run_or_die(cmd: &mut Command, label: &str) {
	let status = cmd.status().unwrap_or_else(|e| panic!("failed to spawn {label}: {e}"));
	if !status.success() {
		eprintln!("{label} exited with {status}");
		std::process::exit(1);
	}
}

fn repo_root() -> PathBuf {
	// cargo -Zscript runs from the directory where the script was invoked.
	env::current_dir().expect("can't read CWD")
}

fn xdg_cache_dir(subdir: &str) -> PathBuf {
	let base = env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{}/.cache", env::var("HOME").expect("HOME not set")));
	let dir = PathBuf::from(base).join("robot_master_train").join(subdir);
	fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("failed to create {}: {e}", dir.display()));
	dir
}

fn zlib_ld_path() -> String {
	let out = Command::new("nix-build")
		.args(["<nixpkgs>", "-A", "zlib", "--no-out-link"])
		.stderr(Stdio::null())
		.output()
		.expect("nix-build failed");
	let nix_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
	format!("{nix_path}/lib")
}

fn latest_model_version(models_dir: &Path) -> u32 {
	fs::read_dir(models_dir)
		.map(|entries| {
			entries
				.flatten()
				.filter_map(|e| {
					let name = e.file_name();
					let s = name.to_str()?;
					let v: u32 = s.strip_prefix("model_v")?.strip_suffix(".onnx")?.parse().ok()?;
					Some(v)
				})
				.max()
				.unwrap_or(0)
		})
		.unwrap_or(0)
}

fn latest_checkpoint(models_dir: &Path) -> Option<PathBuf> {
	fs::read_dir(models_dir)
		.ok()?
		.flatten()
		.filter(|e| e.file_name().to_str().map(|s| s.starts_with("checkpoint_") && s.ends_with(".pt")).unwrap_or(false))
		.max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
		.map(|e| e.path())
}
