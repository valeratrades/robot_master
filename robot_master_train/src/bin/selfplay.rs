#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

use std::{
	env, fs,
	io::Write,
	path::PathBuf,
	sync::atomic::{AtomicU32, Ordering},
	time::Instant,
};

use rand::{SeedableRng, rngs::SmallRng};
use rayon::prelude::*;
use robot_master_arena::algos::rollout::Rollout;
use robot_master_core::game::{GameConfig, GameState};
use robot_master_train::{
	gumbel::GumbelConfig,
	mcts::{Evaluator, RolloutEval},
	nn_eval::NnEval,
	selfplay::{play_game, play_games_batched},
};

fn main() {
	let args = parse_args();

	let start = Instant::now();
	let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
	let config = GumbelConfig {
		n_simulations: args.sims,
		m_actions: args.sims.min(16),
		..Default::default()
	};

	let total_samples = match &args.model {
		Some(model_path) => run_nn(&args, &config, model_path, timestamp, start),
		None => run_rollout(&args, &config, timestamp, start),
	};

	eprintln!();
	let elapsed = start.elapsed().as_secs_f64();
	println!("Done: {total_samples} samples in {elapsed:.1}s  →  {}", args.output.display());
}

/// GPU-batched self-play using the NN evaluator. Single-threaded, batch_size
/// games in-flight simultaneously so the GPU gets large batches.
fn run_nn(args: &Args, config: &GumbelConfig, model_path: &str, timestamp: u64, start: Instant) -> u32 {
	let out_path = args.output.join(format!("selfplay_{timestamp}_00.bin"));
	let mut file = fs::File::create(&out_path).expect("failed to create output file");

	macro_rules! run_batched {
		($N:literal) => {{
			let evaluator = NnEval::new(model_path, $N).expect("failed to load ONNX model");
			let mut rng = SmallRng::seed_from_u64(42);
			let game_batches = play_games_batched::<$N, _, _>(args.games as usize, &evaluator, config, &mut rng, args.batch_size as usize);
			let mut total = 0u32;
			let mut games_done = 0u32;
			for samples in game_batches {
				for sample in &samples {
					file.write_all(&sample.to_bytes()).expect("write failed");
				}
				total += samples.len() as u32;
				games_done += 1;
				if games_done % 10 == 0 || games_done == args.games {
					let elapsed = start.elapsed().as_secs_f64();
					eprint!("\r  {games_done}/{} games  {total} samples  {elapsed:.1}s", args.games);
				}
			}
			total
		}};
	}

	match args.size {
		5 => run_batched!(5),
		7 => run_batched!(7),
		9 => run_batched!(9),
		11 => run_batched!(11),
		_ => panic!("unsupported board size: {}", args.size),
	}
}

/// CPU rollout self-play (no model). Keeps the rayon path since there's no GPU
/// to batch for and parallelism is free here.
fn run_rollout(args: &Args, config: &GumbelConfig, timestamp: u64, start: Instant) -> u32 {
	let games_done = AtomicU32::new(0);
	let samples_done = AtomicU32::new(0);

	let threads = args.threads.min(args.games);
	(0..threads).into_par_iter().for_each(|thread_id| {
		let games_per_thread = (args.games + threads - 1) / threads;
		let out_path = args.output.join(format!("selfplay_{timestamp}_{thread_id:02}.bin"));
		let mut file = fs::File::create(&out_path).expect("failed to create output file");

		let mut rng = SmallRng::seed_from_u64(42 + thread_id as u64);
		let mut thread_samples = 0u32;

		for _ in 0..games_per_thread {
			let samples = match args.size {
				5 => {
					let s = GameState::<5>::new(GameConfig::default(), &mut rng);
					let ev: Box<dyn Evaluator<5>> = Box::new(RolloutEval::new(Rollout {}));
					play_game(&s, &ev, config, &mut rng)
				}
				7 => {
					let s = GameState::<7>::new(GameConfig { size: 7, ..GameConfig::default() }, &mut rng);
					let ev: Box<dyn Evaluator<7>> = Box::new(RolloutEval::new(Rollout {}));
					play_game(&s, &ev, config, &mut rng)
				}
				9 => {
					let s = GameState::<9>::new(GameConfig { size: 9, ..GameConfig::default() }, &mut rng);
					let ev: Box<dyn Evaluator<9>> = Box::new(RolloutEval::new(Rollout {}));
					play_game(&s, &ev, config, &mut rng)
				}
				11 => {
					let s = GameState::<11>::new(GameConfig { size: 11, ..GameConfig::default() }, &mut rng);
					let ev: Box<dyn Evaluator<11>> = Box::new(RolloutEval::new(Rollout {}));
					play_game(&s, &ev, config, &mut rng)
				}
				_ => panic!("unsupported board size: {}", args.size),
			};

			for sample in &samples {
				file.write_all(&sample.to_bytes()).expect("write failed");
			}
			thread_samples += samples.len() as u32;

			let done = games_done.fetch_add(1, Ordering::Relaxed) + 1;
			if done % 10 == 0 || done == args.games {
				let elapsed = start.elapsed().as_secs_f64();
				let total_samples = samples_done.load(Ordering::Relaxed) + thread_samples;
				eprint!("\r  {done}/{} games  {total_samples} samples  {elapsed:.1}s", args.games);
			}
		}

		samples_done.fetch_add(thread_samples, Ordering::Relaxed);
	});

	samples_done.load(Ordering::Relaxed)
}

struct Args {
	games: u32,
	sims: u32,
	output: PathBuf,
	size: u8,
	threads: u32,
	batch_size: u32,
	model: Option<String>,
}

fn parse_args() -> Args {
	let mut games = 500u32;
	let mut sims = 50u32;
	let mut output = xdg_cache_dir("training_data");
	let mut size = 5u8;
	let mut threads = rayon::current_num_threads() as u32;
	let mut batch_size = 128u32;
	let mut model = None;

	let raw: Vec<String> = std::env::args().skip(1).collect();
	let mut i = 0;
	while i < raw.len() {
		match raw[i].as_str() {
			"--games" => {
				games = raw[i + 1].parse().expect("invalid --games");
				i += 2;
			}
			"--sims" => {
				sims = raw[i + 1].parse().expect("invalid --sims");
				i += 2;
			}
			"--output" => {
				output = PathBuf::from(&raw[i + 1]);
				i += 2;
			}
			"--size" => {
				size = raw[i + 1].parse().expect("invalid --size");
				i += 2;
			}
			"--threads" => {
				threads = raw[i + 1].parse().expect("invalid --threads");
				i += 2;
			}
			"--batch-size" => {
				batch_size = raw[i + 1].parse().expect("invalid --batch-size");
				i += 2;
			}
			"--model" => {
				model = Some(raw[i + 1].clone());
				i += 2;
			}
			"--help" | "-h" => {
				println!("Usage: selfplay [--games N] [--sims N] [--output DIR] [--size N] [--threads N] [--batch-size N] [--model PATH]");
				std::process::exit(0);
			}
			other => panic!("unknown argument: {other}"),
		}
	}

	Args {
		games,
		sims,
		output,
		size,
		threads,
		batch_size,
		model,
	}
}

fn xdg_cache_dir(subdir: &str) -> PathBuf {
	let base = env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{}/.cache", env::var("HOME").expect("HOME not set")));
	let dir = PathBuf::from(base).join("robot_master_train").join(subdir);
	fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("failed to create {}: {e}", dir.display()));
	dir
}
