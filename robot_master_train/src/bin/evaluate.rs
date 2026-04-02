#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

//! Pit two ONNX models against each other and report the win rate of the challenger.
//!
//! Each game is played twice with colors swapped to cancel first-mover advantage.
//! Exits with code 0 if the challenger wins rate >= threshold (promote), 1 otherwise (discard).

use std::{
	env,
	sync::atomic::{AtomicU32, Ordering},
};

use board_game::board::{Board as _, Outcome};
use rand::{SeedableRng, rngs::SmallRng};
use rayon::prelude::*;
use robot_master_arena::player::Bot;
use robot_master_core::game::{GameConfig, GameState, Player, PlayerSigned};
use robot_master_train::{
	gumbel::{GumbelConfig, GumbelMcts},
	nn_eval::NnEval,
};

fn main() {
	let args = parse_args();

	// wins/draws/losses from the challenger's perspective
	let challenger_wins = AtomicU32::new(0);
	let draws = AtomicU32::new(0);
	let champion_wins = AtomicU32::new(0);

	// Each "round" is a pair of games (challenger=A then challenger=B), played in parallel.
	// Total games = args.rounds * 2.
	let rounds = (args.games + 1) / 2; // round up so we get at least args.games

	(0..rounds).into_par_iter().for_each(|round_id| {
		let mut rng = SmallRng::seed_from_u64(round_id as u64);

		// Game 1: challenger plays as Player::A (moves first)
		let outcome_a = play_game_5(&args.challenger, &args.champion, args.sims, &mut rng);
		// Game 2: challenger plays as Player::B
		let outcome_b = play_game_5(&args.champion, &args.challenger, args.sims, &mut rng);

		// outcome_a: WonBy(A) means challenger won
		record(&challenger_wins, &draws, &champion_wins, outcome_a, Player::A);
		// outcome_b: WonBy(B) means challenger won
		record(&challenger_wins, &draws, &champion_wins, outcome_b, Player::B);
	});

	let cw = challenger_wins.load(Ordering::Relaxed);
	let d = draws.load(Ordering::Relaxed);
	let chw = champion_wins.load(Ordering::Relaxed);
	let total = cw + d + chw;
	let win_rate = cw as f64 / total as f64;

	println!("challenger: {cw}W / {d}D / {chw}L  over {total} games  win_rate={:.1}%", win_rate * 100.0);

	if win_rate >= args.threshold {
		println!("PROMOTE");
		std::process::exit(0);
	} else {
		println!("DISCARD");
		std::process::exit(1);
	}
}

fn record(cw: &AtomicU32, d: &AtomicU32, chw: &AtomicU32, outcome: Outcome, challenger_color: Player) {
	match outcome {
		Outcome::WonBy(winner) if winner == challenger_color => {
			cw.fetch_add(1, Ordering::Relaxed);
		}
		Outcome::WonBy(_) => {
			chw.fetch_add(1, Ordering::Relaxed);
		}
		Outcome::Draw => {
			d.fetch_add(1, Ordering::Relaxed);
		}
	}
}

/// Play one 5x5 game. `player_a_model` moves first.
fn play_game_5(player_a_model: &str, player_b_model: &str, sims: u32, rng: &mut SmallRng) -> Outcome {
	let make_config = || GumbelConfig {
		n_simulations: sims,
		m_actions: sims.min(16),
		..GumbelConfig::default()
	};
	let mut bot_a = GumbelMcts::new(NnEval::new(player_a_model, 5, false).expect("failed to load player A model"), make_config());
	let mut bot_b = GumbelMcts::new(NnEval::new(player_b_model, 5, false).expect("failed to load player B model"), make_config());
	let mut state = GameState::<5>::new(GameConfig::default(), rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);

	while state.outcome().is_none() {
		let mv = if state.turn == Player::A { bot_a.choose_move(&state) } else { bot_b.choose_move(&state) };
		state.play(mv).expect("bot produced illegal move");
	}

	state.outcome().expect("game must be finished")
}

struct Args {
	challenger: String,
	champion: String,
	games: u32,
	threshold: f64,
	sims: u32,
}

fn parse_args() -> Args {
	let mut challenger = None;
	let mut champion = None;
	let mut games = 400u32;
	let mut threshold = 0.55f64;
	let mut sims = 200u32;

	let raw: Vec<String> = env::args().skip(1).collect();
	let mut i = 0;
	while i < raw.len() {
		match raw[i].as_str() {
			"--challenger" => {
				challenger = Some(raw[i + 1].clone());
				i += 2;
			}
			"--champion" => {
				champion = Some(raw[i + 1].clone());
				i += 2;
			}
			"--games" => {
				games = raw[i + 1].parse().expect("invalid --games");
				i += 2;
			}
			"--threshold" => {
				threshold = raw[i + 1].parse().expect("invalid --threshold");
				i += 2;
			}
			"--sims" => {
				sims = raw[i + 1].parse().expect("invalid --sims");
				i += 2;
			}
			"--help" | "-h" => {
				println!("Usage: evaluate --challenger PATH --champion PATH [--games N] [--threshold F] [--sims N]");
				println!("  --games      total games to play (will be rounded up to even, default 400)");
				println!("  --threshold  win rate required to promote challenger (default 0.55)");
				println!("  --sims       Gumbel simulations per move (default 200)");
				println!("Exit code: 0 = promote, 1 = discard");
				std::process::exit(0);
			}
			other => panic!("unknown argument: {other}"),
		}
	}

	Args {
		challenger: challenger.expect("--challenger is required"),
		champion: champion.expect("--champion is required"),
		games,
		threshold,
		sims,
	}
}
