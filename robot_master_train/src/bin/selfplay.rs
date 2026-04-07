#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

use std::{
	env, fs,
	io::Write,
	path::PathBuf,
	sync::{
		Arc, Mutex,
		atomic::{AtomicU32, Ordering},
	},
	time::Instant,
};

use clap::Parser;
use rand::{SeedableRng, rngs::SmallRng};
use rayon::prelude::*;
use robot_master_arena::algos::{PlayerKind, rollout::Rollout};
use robot_master_core::game::{GameConfig, GameState, Player, PlayerSigned};
use robot_master_train::{
	gumbel::GumbelConfig,
	mcts::{Evaluator, RolloutEval},
	nn_eval::NnEval,
	player_kind::kind_into_rollout_evaluator,
	selfplay::{play_game, play_games_batched},
};

#[derive(Parser)]
#[command(about = "Self-play data generation for AlphaZero training")]
struct Args {
	/// Number of games to play.
	#[arg(long, default_value_t = 500)]
	games: u32,
	/// Gumbel simulations per move.
	#[arg(long, default_value_t = 50)]
	sims: u32,
	/// Output directory for .bin sample files.
	#[arg(long, default_value_os_t = xdg_cache_dir("training_data"))]
	output: PathBuf,
	/// Board size (5, 7, 9, or 11).
	#[arg(long, default_value_t = 5)]
	size: u8,
	/// Number of rayon threads (rollout path only; ignored with --model).
	#[arg(long, default_value_t = rayon::current_num_threads() as u32)]
	threads: u32,
	/// Games in-flight for GPU batching (NN path only).
	#[arg(long, default_value_t = 128)]
	batch_size: u32,
	/// Path to ONNX model. If omitted, uses CPU rollout (no NN).
	#[arg(long)]
	model: Option<String>,
	/// Use CPU for NN inference instead of CUDA. Uses sequential rayon
	/// parallelism (one evaluate call per leaf, all threads independent) which
	/// is faster than GPU batching at small board sizes (5×5, 7×7). See
	/// docs/perf.md for per-board-size benchmarks. Has no effect without
	/// --model (rollout path is always CPU).
	#[arg(long)]
	force_cpu: bool,
	/// Hide opponent's hand (information-hidden mode).
	#[arg(long)]
	hide: bool,
	/// Bot spec to use as the rollout policy instead of the default `Rollout` bot.
	/// Sims in the spec are ignored — the bot drives `RolloutEval` directly.
	/// Mutually exclusive with --model.
	#[arg(long)]
	supervise_bot: Option<String>,
}

fn main() {
	let args = Args::parse();

	let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
	let config = GumbelConfig {
		n_simulations: args.sims,
		m_actions: args.sims.min(16),
		..Default::default()
	};

	if args.model.is_some() && args.supervise_bot.is_some() {
		eprintln!("error: --supervise-bot cannot be used with --model");
		std::process::exit(1);
	}

	let (total_samples, elapsed, init_elapsed) = match &args.model {
		Some(model_path) if args.force_cpu => run_nn_sequential(&args, &config, model_path, timestamp),
		Some(model_path) => run_nn(&args, &config, model_path, timestamp),
		None => {
			let start = Instant::now();
			let samples = run_rollout(&args, &config, timestamp, &start);
			(samples, start.elapsed().as_secs_f64(), 0.0f64)
		}
	};

	eprintln!();
	println!("Done: {total_samples} samples in {elapsed:.1}s  →  {}  (init {init_elapsed:.1}s)", args.output.display());
}

/// GPU-batched self-play using the NN evaluator. Single-threaded, batch_size
/// games in-flight simultaneously so the GPU gets large batches.
///
/// Returns `(total_samples, game_elapsed_secs, init_elapsed_secs)`.
fn run_nn(args: &Args, config: &GumbelConfig, model_path: &str, timestamp: u64) -> (u32, f64, f64) {
	let out_path = args.output.join(format!("selfplay_{timestamp}_00.bin"));
	let mut file = fs::File::create(&out_path).expect("failed to create output file");

	macro_rules! run_batched {
		($N:literal) => {{
			let init_start = Instant::now();
			let evaluator = NnEval::try_new(model_path, $N, args.force_cpu).expect("failed to load ONNX model");
			let init_elapsed = init_start.elapsed().as_secs_f64();
			let game_start = Instant::now();
			let mut rng = SmallRng::seed_from_u64(42);
			let game_config = GameConfig { size: $N, hide: args.hide };
			let game_batches = play_games_batched::<$N, _, _>(args.games as usize, &evaluator, config, &mut rng, args.batch_size as usize, game_config);
			let mut total = 0u32;
			let mut games_done = 0u32;
			for samples in game_batches {
				for sample in &samples {
					file.write_all(&sample.to_bytes()).expect("write failed");
				}
				total += samples.len() as u32;
				games_done += 1;
				if games_done % 10 == 0 || games_done == args.games {
					let elapsed = game_start.elapsed().as_secs_f64();
					eprint!("\r  {games_done}/{} games  {total} samples  {elapsed:.1}s", args.games);
				}
			}
			(total, game_start.elapsed().as_secs_f64(), init_elapsed)
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

/// Sequential (non-batched) NN self-play via rayon threads. One `evaluate`
/// call per MCTS leaf per game — the pre-batching baseline. Benchmarking only.
fn run_nn_sequential(args: &Args, config: &GumbelConfig, model_path: &str, timestamp: u64) -> (u32, f64, f64) {
	let init_start = Instant::now();

	macro_rules! run_seq {
		($N:literal) => {{
			let init_elapsed = init_start.elapsed().as_secs_f64();
			let games_done = AtomicU32::new(0);
			let samples_done = AtomicU32::new(0);
			let game_start = Instant::now();

			let out_path = args.output.join(format!("selfplay_{timestamp}_00.bin"));
			let file = Arc::new(Mutex::new(fs::File::create(&out_path).expect("failed to create output file")));

			let threads = args.threads.min(args.games);
			(0..threads).into_par_iter().for_each(|thread_id| {
				let evaluator = NnEval::try_new(model_path, $N, args.force_cpu).expect("failed to load ONNX model");
				let games_per_thread = (args.games + threads - 1) / threads;
				let mut rng = SmallRng::seed_from_u64(42 + thread_id as u64);
				let mut thread_samples = 0u32;

				let game_config = GameConfig { size: $N, hide: args.hide };
				for _ in 0..games_per_thread {
					let s = GameState::<$N>::new(game_config, &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
					let samples = play_game(&s, &evaluator, config, &mut rng);
					{
						let mut f = file.lock().unwrap();
						for sample in &samples {
							f.write_all(&sample.to_bytes()).expect("write failed");
						}
					}
					thread_samples += samples.len() as u32;

					let done = games_done.fetch_add(1, Ordering::Relaxed) + 1;
					if done % 10 == 0 || done == args.games {
						let elapsed = game_start.elapsed().as_secs_f64();
						let total_samples = samples_done.load(Ordering::Relaxed) + thread_samples;
						eprint!("\r  {done}/{} games  {total_samples} samples  {elapsed:.1}s", args.games);
					}
				}

				samples_done.fetch_add(thread_samples, Ordering::Relaxed);
			});

			(samples_done.load(Ordering::Relaxed), game_start.elapsed().as_secs_f64(), init_elapsed)
		}};
	}

	match args.size {
		5 => run_seq!(5),
		7 => run_seq!(7),
		9 => run_seq!(9),
		11 => run_seq!(11),
		_ => panic!("unsupported board size: {}", args.size),
	}
}

/// CPU rollout self-play (no model). Keeps the rayon path since there's no GPU
/// to batch for and parallelism is free here.
fn run_rollout(args: &Args, config: &GumbelConfig, timestamp: u64, start: &Instant) -> u32 {
	let games_done = AtomicU32::new(0);
	let samples_done = AtomicU32::new(0);

	let out_path = args.output.join(format!("selfplay_{timestamp}_00.bin"));
	let file = Arc::new(Mutex::new(fs::File::create(&out_path).expect("failed to create output file")));

	let threads = args.threads.min(args.games);
	(0..threads).into_par_iter().for_each(|thread_id| {
		let games_per_thread = (args.games + threads - 1) / threads;

		let mut rng = SmallRng::seed_from_u64(42 + thread_id as u64);
		let mut thread_samples = 0u32;

		for _ in 0..games_per_thread {
			let samples = match args.size {
				5 => {
					let s = GameState::<5>::new(GameConfig { size: 5, hide: args.hide }, &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
					let ev = make_rollout_evaluator::<5>(&args.supervise_bot);
					play_game(&s, &ev, config, &mut rng)
				}
				7 => {
					let s = GameState::<7>::new(GameConfig { size: 7, hide: args.hide }, &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
					let ev = make_rollout_evaluator::<7>(&args.supervise_bot);
					play_game(&s, &ev, config, &mut rng)
				}
				9 => {
					let s = GameState::<9>::new(GameConfig { size: 9, hide: args.hide }, &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
					let ev = make_rollout_evaluator::<9>(&args.supervise_bot);
					play_game(&s, &ev, config, &mut rng)
				}
				11 => {
					let s = GameState::<11>::new(GameConfig { size: 11, hide: args.hide }, &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
					let ev = make_rollout_evaluator::<11>(&args.supervise_bot);
					play_game(&s, &ev, config, &mut rng)
				}
				_ => panic!("unsupported board size: {}", args.size),
			};

			{
				let mut f = file.lock().unwrap();
				for sample in &samples {
					f.write_all(&sample.to_bytes()).expect("write failed");
				}
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

fn make_rollout_evaluator<const N: usize>(spec: &Option<String>) -> Box<dyn Evaluator<N>>
where
	[(); N * N]:,
	[(); N + 1]:, {
	match spec {
		None => Box::new(RolloutEval::new(Rollout {})),
		Some(s) => {
			let kind: PlayerKind = s.parse().unwrap_or_else(|_| {
				eprintln!("error: invalid --supervise-bot spec: {s:?}");
				std::process::exit(1);
			});
			kind_into_rollout_evaluator::<N>(&kind)
		}
	}
}

fn xdg_cache_dir(subdir: &str) -> PathBuf {
	let base = env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{}/.cache", env::var("HOME").expect("HOME not set")));
	let dir = PathBuf::from(base).join("robot_master_train").join(subdir);
	fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("failed to create {}: {e}", dir.display()));
	dir
}
