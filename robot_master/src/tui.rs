use std::io::{self, BufRead, Write};

use rand::{SeedableRng, rngs::SmallRng};
use robot_master_arena::{
	db::RatingDb,
	match_::{Match, MatchResult},
	player::Player,
	rating::{self, EloRating, Outcome},
};
use robot_master_core::{
	board::{Board, Pos},
	cards::{CardValue, Hand},
	game::{GameConfig, GameState, Move, PlayerId},
};

use crate::algos;

pub fn run(config: GameConfig, p1_name: &str, p2_name: &str, rating_db: &dyn RatingDb) {
	let stdout_handle = io::stdout();
	let mut stdout = stdout_handle.lock();
	let stdin_handle = io::stdin();
	let mut stdin = stdin_handle.lock();
	let mut rng = SmallRng::from_os_rng();

	match config.size {
		5 => run_sized::<5>(config, p1_name, p2_name, rating_db, &mut rng, &mut stdout, &mut stdin),
		7 => run_sized::<7>(config, p1_name, p2_name, rating_db, &mut rng, &mut stdout, &mut stdin),
		9 => run_sized::<9>(config, p1_name, p2_name, rating_db, &mut rng, &mut stdout, &mut stdin),
		11 => run_sized::<11>(config, p1_name, p2_name, rating_db, &mut rng, &mut stdout, &mut stdin),
		n => panic!("unsupported board size {n}"),
	}
}

/// Returns the move and erases all its own output, so the caller can re-display the board uniformly.
fn read_manual_move<const N: usize>(board: &Board<N>, hand: &Hand, name: &str, stdout: &mut impl Write, stdin: &mut impl BufRead) -> Move
where
	[(); N * N]:, {
	let mut lines_to_erase = 0usize;
	let mut warning: Option<String> = None;
	loop {
		if lines_to_erase > 0 {
			write!(stdout, "\x1B[{lines_to_erase}A\x1B[J").unwrap();
		}
		let board_str = board.to_string();
		let board_lines = board_str.lines().count();
		write!(stdout, "{board_str}\n").unwrap();
		writeln!(stdout, "{name}, votre main : {hand}").unwrap();
		let mut prompt_lines = 0usize;
		let base_lines = board_lines + 1;

		if let Some(ref w) = warning {
			writeln!(stdout, "\x1B[33mWARNING: {w}\x1B[0m").unwrap();
		}
		let total_base = base_lines + if warning.take().is_some() { 1 } else { 0 };

		write!(stdout, "Choisissez une carte : ").unwrap();
		stdout.flush().unwrap();
		let mut line = String::new();
		stdin.read_line(&mut line).unwrap();
		prompt_lines += 1;
		let carte: u8 = match line.trim().parse() {
			Ok(v) => v,
			Err(_) => {
				warning = Some("expected a number".into());
				lines_to_erase = total_base + prompt_lines;
				continue;
			}
		};
		if hand.count(CardValue(carte)) == 0 {
			warning = Some(format!("no card {carte} in hand"));
			lines_to_erase = total_base + prompt_lines;
			continue;
		}

		write!(stdout, "Ligne : ").unwrap();
		stdout.flush().unwrap();
		let mut line = String::new();
		stdin.read_line(&mut line).unwrap();
		prompt_lines += 1;
		let row: u8 = match line.trim().parse() {
			Ok(v) => v,
			Err(_) => {
				warning = Some("expected a number".into());
				lines_to_erase = total_base + prompt_lines;
				continue;
			}
		};

		write!(stdout, "Colonne : ").unwrap();
		stdout.flush().unwrap();
		let mut line = String::new();
		stdin.read_line(&mut line).unwrap();
		prompt_lines += 1;
		let col: u8 = match line.trim().parse() {
			Ok(v) => v,
			Err(_) => {
				warning = Some("expected a number".into());
				lines_to_erase = total_base + prompt_lines;
				continue;
			}
		};

		let pos = Pos { row, col };
		if !board.is_playable(pos) {
			if row as usize >= N || col as usize >= N {
				warning = Some(format!("({row},{col}) is out of bounds"));
			} else if !board.is_empty(pos) {
				warning = Some(format!("({row},{col}) is already occupied"));
			} else {
				warning = Some(format!("({row},{col}) has no adjacent card"));
			}
			lines_to_erase = total_base + prompt_lines;
			continue;
		}

		// Erase everything we printed so the main loop can re-display uniformly
		let total = total_base + prompt_lines;
		write!(stdout, "\x1B[{total}A\x1B[J").unwrap();

		return Move { pos, card: CardValue(carte) };
	}
}

fn is_manual(name: &str) -> bool {
	matches!(name, "m" | "manual")
}

#[deprecated(note = "literally just derive with strum or serde")]
fn player_display_name(name: &str, player_id: PlayerId) -> String {
	let side = match player_id {
		PlayerId::Cols => "Cols",
		PlayerId::Rows => "Rows",
	};
	format!("{name} ({side})")
}

fn run_sized<const N: usize>(config: GameConfig, p1_name: &str, p2_name: &str, rating_db: &dyn RatingDb, rng: &mut SmallRng, stdout: &mut impl Write, stdin: &mut impl BufRead)
where
	[(); N * N]:, {
	let game: GameState<N> = GameState::new(config, rng);
	let p1_manual = is_manual(p1_name);
	let p2_manual = is_manual(p2_name);

	let p1_display = player_display_name(p1_name, PlayerId::Cols);
	let p2_display = player_display_name(p2_name, PlayerId::Rows);

	// Resolve players. Manual players get a placeholder that panics on choose_move.
	let p1: Box<dyn Player<N>> = algos::resolve::<N>(p1_name).unwrap_or_else(|| Box::new(algos::manual::Manual::new(p1_name)));
	let p2: Box<dyn Player<N>> = algos::resolve::<N>(p2_name).unwrap_or_else(|| Box::new(algos::manual::Manual::new(p2_name)));

	let mut match_ = Match::new(game, p1, p2);

	// Show initial board
	let board_str = match_.state().board.to_string();
	writeln!(stdout, "{board_str}").unwrap();
	let mut prev_lines = board_str.lines().count() + 1;

	let result = loop {
		let state = match_.state();
		let current_is_manual = match state.turn {
			PlayerId::Cols => p1_manual,
			PlayerId::Rows => p2_manual,
		};
		let current_name = match state.turn {
			PlayerId::Cols => &p1_display,
			PlayerId::Rows => &p2_display,
		};

		// Clear previous turn's output
		if prev_lines > 0 {
			write!(stdout, "\x1B[{prev_lines}A\x1B[J").unwrap();
			prev_lines = 0;
		}

		let external_move = if current_is_manual {
			let player_idx = state.turn as usize;
			Some(read_manual_move(&state.board, &state.hands[player_idx], current_name, stdout, stdin))
		} else {
			None
		};

		match match_.next(external_move) {
			Ok(state) => {
				let board_str = state.board.to_string();
				let output = format!("au tour de {current_name}\n{board_str}");
				writeln!(stdout, "{output}").unwrap();
				prev_lines = output.lines().count() + 1;
			}
			Err(result) => break result,
		}
	};

	// Clear last turn display, show final board + results
	if prev_lines > 0 {
		write!(stdout, "\x1B[{prev_lines}A\x1B[J").unwrap();
	}

	let board_str = match_.state().board.to_string();
	writeln!(stdout, "{board_str}").unwrap();

	display_result::<N>(&result, &p1_display, &p2_display, stdout);
	update_elo(&result, rating_db, stdout);
}

fn display_result<const N: usize>(result: &MatchResult, p1_display: &str, p2_display: &str, stdout: &mut impl Write)
where
	[(); N * N]:, {
	let verdict = match result.p1_score.cmp(&result.p2_score) {
		std::cmp::Ordering::Greater => format!("{p1_display} wins."),
		std::cmp::Ordering::Less => format!("{p2_display} wins."),
		std::cmp::Ordering::Equal => "draw.".into(),
	};

	writeln!(
		stdout,
		"{p1_display}: score {} (column {})\n{p2_display}: score {} (row {})\n{verdict}",
		result.p1_score, result.p1_weak_line, result.p2_score, result.p2_weak_line
	)
	.unwrap();
}

fn update_elo(result: &MatchResult, rating_db: &dyn RatingDb, stdout: &mut impl Write) {
	let mut ratings = rating_db.load_ratings();

	let outcome = match result.p1_score.cmp(&result.p2_score) {
		std::cmp::Ordering::Greater => Outcome::P1Win,
		std::cmp::Ordering::Less => Outcome::P2Win,
		std::cmp::Ordering::Equal => Outcome::Draw,
	};

	// Ensure both entries exist, then clone out for the pure update.
	ratings.entry(result.p1_id).or_insert_with(EloRating::default);
	ratings.entry(result.p2_id).or_insert_with(EloRating::default);
	let r1 = ratings[&result.p1_id].clone();
	let r2 = ratings[&result.p2_id].clone();
	let old_r1 = r1.rating;
	let old_r2 = r2.rating;

	let (new_r1, new_r2) = rating::elo_update(&r1, &r2, outcome, rating::DEFAULT_K);
	ratings.insert(result.p1_id, new_r1.clone());
	ratings.insert(result.p2_id, new_r2.clone());

	rating_db.save_ratings(&ratings);

	let delta1 = new_r1.rating - old_r1;
	let delta2 = new_r2.rating - old_r2;
	let sign = |d: f64| if d >= 0.0 { "+" } else { "" };
	writeln!(
		stdout,
		"\nElo: {} {:.0} ({}{:.0}) | {} {:.0} ({}{:.0})",
		result.p1_id,
		new_r1.rating,
		sign(delta1),
		delta1,
		result.p2_id,
		new_r2.rating,
		sign(delta2),
		delta2
	)
	.unwrap();
}
