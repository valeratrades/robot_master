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
#[command(about = "AlphaZero Transformer training loop: selfplay → train → export → repeat")]
struct Args {
	/// Generation name — all data/checkpoints/models are scoped under this label (e.g. "v1", "transformer_big")
	generation: String,
	/// Number of selfplay → train → export iterations
	#[arg(long, default_value = "20")]
	iterations: u32,
	/// Self-play games per iteration
	#[arg(long, default_value = "200")]
	games: u32,
	/// Gumbel simulations per move during self-play
	#[arg(long, default_value = "25")]
	sims: u32,
	/// Board size (must match the selfplay binary and model architecture)
	#[arg(long, default_value = "5")]
	size: u32,
	/// Pass --force-cpu to selfplay (sequential rayon, faster at 5×5/7×7).
	#[arg(long)]
	force_cpu: bool,
	/// Hide opponent's hand (information-hidden mode).
	#[arg(long)]
	hide: bool,
}

fn main() {
	let args = Args::parse();

	let train_steps = (args.games / 2).max(1);
	let total_steps = train_steps * args.iterations;
	let value_weight = 1.0f64;
	let hide_label = if args.hide { "hide" } else { "show" };
	let run_id = format!("{}:g{}:s{}/{}x{}_{}", args.generation, args.games, args.sims, args.size, args.size, hide_label);
	let data_dir = xdg_cache_dir(&format!("{run_id}/training_data"));
	fs::create_dir_all(&data_dir).unwrap_or_else(|e| panic!("failed to create {}: {e}", data_dir.display()));
	let models_out = xdg_cache_dir(&format!("{run_id}/models"));

	let zlib_path = zlib_ld_path();
	let repo_root = repo_root();

	let selfplay_bin = repo_root.join("target/release/selfplay");
	let train_py = repo_root.join("training/train.py");
	let export_py = repo_root.join("training/export_onnx.py");

	let mut version = latest_model_version(&models_out);
	let current_model: Option<PathBuf> = if version > 0 {
		let p = models_out.join(format!("model_v{version}.onnx"));
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

	eprintln!("Building selfplay binary...");
	run_or_die(
		Command::new("cargo")
			.args(["b", "--release", "-p", "robot_master_train", "--bin", "selfplay"])
			.current_dir(&repo_root),
		"cargo build selfplay",
	);

	let total_start = Instant::now();

	let bar = ProgressBar::new(args.iterations as u64);
	bar.set_style(ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} iterations  elapsed {elapsed_precise}  eta {eta_precise}").unwrap());

	for i in 1..=args.iterations {
		let iter_start = Instant::now();
		eprintln!("\n━━━ Iteration {i}/{} ━━━", args.iterations);

		// 1. Self-play
		let mut selfplay_cmd = Command::new(&selfplay_bin);
		selfplay_cmd
			.args(["--games", &args.games.to_string()])
			.args(["--sims", &args.sims.to_string()])
			.args(["--size", &args.size.to_string()])
			.args(["--output", data_dir.to_str().unwrap()])
			.current_dir(&repo_root);
		if let Some(ref model) = current_model {
			selfplay_cmd.args(["--model".as_ref(), model.to_str().unwrap()]);
		}
		if args.force_cpu {
			selfplay_cmd.arg("--force-cpu");
		}
		if args.hide {
			selfplay_cmd.arg("--hide");
		}
		let sp_start = Instant::now();
		run_or_die(&mut selfplay_cmd, "selfplay");
		eprintln!("  [1/3] Self-play ({} games, {} sims): done ({:.1}s)", args.games, args.sims, sp_start.elapsed().as_secs_f64());

		// 2. Train
		eprint!("  [2/3] Training ({} steps)... ", train_steps);
		let train_start = Instant::now();
		let resume_checkpoint = latest_checkpoint(&models_out);
		let output = retry_on_clock_error(|| {
			let mut cmd = Command::new("python");
			cmd.args([train_py.to_str().unwrap()])
				.args(["--model", "transformer"])
				.args(["--lr", "1e-3"])
				.args(["--data-dir", data_dir.to_str().unwrap()])
				.args(["--output-dir", models_out.to_str().unwrap()])
				.args(["--board-size", &args.size.to_string()])
				.args(["--steps", &train_steps.to_string()])
				.args(["--total-steps", &total_steps.to_string()])
				.args(["--max-iters", &replay_buffer_iters(args.iterations).to_string()])
				.args(["--value-weight", &format!("{value_weight:.4}")])
				.env("LD_LIBRARY_PATH", &zlib_path)
				.current_dir(&repo_root);
			if let Some(ref ckpt) = resume_checkpoint {
				cmd.args(["--resume", ckpt.to_str().unwrap()]);
			}
			cmd.output().expect("failed to run train.py")
		});
		if !output.status.success() {
			eprintln!("FAILED");
			eprintln!("{}", String::from_utf8_lossy(&output.stderr));
			std::process::exit(1);
		}
		let stdout = String::from_utf8_lossy(&output.stdout);
		let train_summary = stdout.lines().filter(|l| l.starts_with("Steps")).last().unwrap_or("(no output)");
		eprintln!("done ({:.1}s)  {train_summary}", train_start.elapsed().as_secs_f64());

		// 3. Export ONNX
		version += 1;
		let onnx_path = models_out.join(format!("model_v{version}.onnx"));
		let checkpoint = latest_checkpoint(&models_out).expect("no checkpoint found after training");
		eprint!("  [3/3] Exporting {} → model_v{version}.onnx... ", checkpoint.file_name().unwrap().to_str().unwrap());
		let export_start = Instant::now();
		let export_out = retry_on_clock_error(|| {
			Command::new("python")
				.args([export_py.to_str().unwrap()])
				.args(["--checkpoint", checkpoint.to_str().unwrap()])
				.args(["--output", onnx_path.to_str().unwrap()])
				.args(["--board-size", &args.size.to_string()])
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
	eprintln!("To run in the arena:  robot_master arena --models-dir {} tourney rating 200", models_out.display());
}

fn replay_buffer_iters(total_iterations: u32) -> u32 {
	(3.0 * (total_iterations as f64).ln().ceil()) as u32
}

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
	env::current_dir().expect("can't read CWD")
}

fn xdg_cache_dir(subdir: &str) -> PathBuf {
	let base = env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{}/.cache", env::var("HOME").expect("HOME not set")));
	PathBuf::from(base).join("robot_master_train").join(subdir)
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

fn latest_model_version(models_out: &Path) -> u32 {
	fs::read_dir(models_out)
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

fn latest_checkpoint(models_out: &Path) -> Option<PathBuf> {
	fs::read_dir(models_out)
		.ok()?
		.flatten()
		.filter(|e| e.file_name().to_str().map(|s| s.ends_with(".pt")).unwrap_or(false))
		.max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
		.map(|e| e.path())
}
