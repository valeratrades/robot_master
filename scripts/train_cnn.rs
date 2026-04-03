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
	/// Gumbel simulations per move during self-play
	#[arg(long, default_value = "25")]
	sims: u32,
	/// Board size (must match the selfplay binary and model architecture)
	#[arg(long, default_value = "5")]
	size: u32,
	/// Pass --force-cpu to selfplay (sequential rayon, faster at 5×5/7×7).
	#[arg(long)]
	force_cpu: bool,
	/// Hide opponent's hand (information-hidden mode). Determines `_hide` vs `_show` in run path.
	#[arg(long)]
	hide: bool,
	/// Supervised pre-training spec (e.g. `rollout|v50`). Controls both:
	/// selfplay data generation (uses this bot until NN beats it >68% over 32 games),
	/// and eval matches run every 10 versions to detect the threshold.
	/// If omitted, starts NN selfplay immediately with no eval.
	#[arg(long)]
	supervise: Option<String>,
}

fn main() {
	let args = Args::parse();

	// MiniZero: steps proportional to games collected, ratio 1:10 (final.tex line 299).
	let train_steps = (args.games / 10).max(1);
	let total_steps = train_steps * args.iterations;
	let hide_label = if args.hide { "hide" } else { "show" };
	let run_id = format!("{}:g{}:s{}/{}x{}_{}", args.generation, args.games, args.sims, args.size, args.size, hide_label);
	let data_dir = xdg_cache_dir(&format!("{run_id}/training_data"));
	fs::create_dir_all(&data_dir).unwrap_or_else(|e| panic!("failed to create {}: {e}", data_dir.display()));
	let models_out = xdg_cache_dir(&format!("{run_id}/models"));

	// Detect the zlib path once (needed for Python/numpy on NixOS)
	let zlib_path = zlib_ld_path();
	let repo_root = repo_root();

	let selfplay_bin = repo_root.join("target/release/selfplay");
	let robot_master_bin = repo_root.join("target/release/robot_master");
	let train_py = repo_root.join("training/train.py");
	let export_py = repo_root.join("training/export_onnx.py");

	// Find the highest existing model version to resume from
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
	// True until the NN beats the supervise-bot >68% in eval; then we switch to NN selfplay.
	// On resume, if a model already exists the supervised phase is already past.
	let mut use_supervise_bot = args.supervise.is_some() && current_model.is_none();

	// Build selfplay binary upfront
	eprintln!("Building selfplay binary...");
	run_or_die(
		Command::new("cargo")
			.args(["b", "--release", "-p", "robot_master_train", "--bin", "selfplay"])
			.current_dir(&repo_root),
		"cargo build selfplay",
	);
	if args.supervise.is_some() {
		eprintln!("Building robot_master binary (needed for --supervise eval)...");
		run_or_die(
			Command::new("cargo").args(["b", "--release", "-p", "robot_master"]).current_dir(&repo_root),
			"cargo build robot_master",
		);
	}

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
		if use_supervise_bot && current_model.is_none() {
			if let Some(ref spec) = args.supervise {
				selfplay_cmd.args(["--supervise-bot", spec]);
			}
		}
		let sp_start = Instant::now();
		run_or_die(&mut selfplay_cmd, "selfplay");
		eprintln!("  [1/3] Self-play ({} games, {} sims): done ({:.1}s)", args.games, args.sims, sp_start.elapsed().as_secs_f64());

		// 2. Train
		eprint!("  [2/3] Training ({} steps)... ", train_steps);
		let train_start = Instant::now();
		// Resume from the previous iteration's checkpoint to preserve SGD momentum across
		// iterations. AlphaZero and MiniZero train continuously — cold-restarting the optimizer
		// every iteration discards accumulated momentum (m=0.9), effectively halving the
		// usable LR and slowing convergence. See docs/references/MiniZero/final.tex §IV-A.
		let resume_checkpoint = latest_checkpoint(&models_out);
		let output = retry_on_clock_error(|| {
			let mut cmd = Command::new("python");
			cmd.args([train_py.to_str().unwrap()])
				.args(["--data-dir", data_dir.to_str().unwrap()])
				.args(["--output-dir", models_out.to_str().unwrap()])
				.args(["--board-size", &args.size.to_string()])
				.args(["--steps", &train_steps.to_string()])
				.args(["--total-steps", &total_steps.to_string()])
				.args(["--max-iters", &replay_buffer_iters(args.iterations).to_string()])
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
		// Extract last loss line from stdout
		let stdout = String::from_utf8_lossy(&output.stdout);
		let train_summary = stdout.lines().filter(|l| l.starts_with("Steps")).last().unwrap_or("(no output)");
		eprintln!("done ({:.1}s)  {train_summary}", train_start.elapsed().as_secs_f64());

		// 3. Export ONNX — always promote, AlphaZero-style (no separate evaluation step)
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

		// Eval every 10 versions against the supervised baseline
		if version % 10 == 0 {
			if let Some(ref spec) = args.supervise {
				let hide_flag = if args.hide { "h" } else { "v" };
				let model_id = format!("onnx:model_v{}|g{}|s{}|h{}", version, args.sims, args.size, hide_flag);
				let no_priors_spec = format!("{},{}", spec, model_id);

				eprint!("  [eval] model_v{version} vs {spec} (32 games)... ");
				let eval_start = Instant::now();
				let output = Command::new(&robot_master_bin)
					.args(["--models-dir", models_out.to_str().unwrap()])
					.args(["arena", "--no-priors", &no_priors_spec])
					.args(["tourney", "--json", "swiss", "32"])
					.current_dir(&repo_root)
					.output()
					.expect("failed to run arena eval");

				if output.status.success() {
					let json = String::from_utf8_lossy(&output.stdout);
					if let Some((wins, total)) = parse_eval_json(&json, &model_id) {
						let pct = wins as f64 / total as f64 * 100.0;
						let threshold_note = if pct > 68.0 {
							if use_supervise_bot {
								use_supervise_bot = false;
								" ✓ above threshold — switching to NN selfplay"
							} else {
								" ✓ above threshold"
							}
						} else {
							""
						};
						eprintln!("done ({:.1}s)  {wins}/{total} ({pct:.0}%){threshold_note}", eval_start.elapsed().as_secs_f64(),);
					} else {
						eprintln!("done ({:.1}s)  (could not parse result)", eval_start.elapsed().as_secs_f64());
					}
				} else {
					eprintln!("FAILED");
					eprintln!("{}", String::from_utf8_lossy(&output.stderr));
				}
			}
		}

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

/// How many of the most-recent iteration files to feed into training (replay buffer window).
///
/// Formula: 3 * ceil(ln(total_iterations))
/// Yields ~9 for 20 iters, ~12 for 50, ~15 for 100, ~18 for 300, ~33 for 25k.
/// Q: empirically unverified at this scale — AGZ/MiniZero both converge on a flat ~20x-games ratio; this log formula grows slower, biasing toward recency. See docs/references/replay_buffer_sizing.md.
fn replay_buffer_iters(total_iterations: u32) -> u32 {
	(3.0 * (total_iterations as f64).ln().ceil()) as u32
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

/// Parse the JSON array from `arena tourney --json` and extract wins/games for `id`.
/// JSON format: `[{"id":"rollout","wins":10,"games":32},{"id":"onnx:...","wins":22,"games":32}]`
fn parse_eval_json(json: &str, id: &str) -> Option<(u32, u32)> {
	let needle = format!(r#""id":"{}""#, id);
	// Find the object containing our id — search from the right to handle any prefix matches
	let id_pos = json.rfind(&needle)?;
	// Walk back to find the opening `{` of this object
	let obj_start = json[..id_pos].rfind('{').unwrap_or(id_pos);
	let chunk = &json[obj_start..];
	let wins = extract_u32(chunk, "wins")?;
	let games = extract_u32(chunk, "games")?;
	Some((wins, games))
}

fn extract_u32(s: &str, key: &str) -> Option<u32> {
	let needle = format!(r#""{key}":"#);
	let pos = s.find(&needle)? + needle.len();
	s[pos..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}
