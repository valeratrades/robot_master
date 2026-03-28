use std::io::{self, BufRead, Write};

use rand::rngs::SmallRng;
use robot_master_arena::{
	algos,
	db::RatingDb,
	match_::{Match, MatchResult},
};
use robot_master_core::{
	board::{Board, Pos},
	cards::{CardValue, Hand},
	game::{GameConfig, GameState, Move, PlayerId},
};

pub fn run(config: GameConfig, p1_name: &str, p2_name: &str, rating_db: Box<dyn RatingDb>) {
	let stdout_handle = io::stdout();
	let mut stdout = stdout_handle.lock();
	let stdin_handle = io::stdin();
	let mut stdin = stdin_handle.lock();
	let mut rng = rand::make_rng();

	match config.size {
		5 => run_sized::<5>(config, p1_name, p2_name, &mut rng, &mut stdout, &mut stdin, rating_db),
		7 => run_sized::<7>(config, p1_name, p2_name, &mut rng, &mut stdout, &mut stdin, rating_db),
		9 => run_sized::<9>(config, p1_name, p2_name, &mut rng, &mut stdout, &mut stdin, rating_db),
		11 => run_sized::<11>(config, p1_name, p2_name, &mut rng, &mut stdout, &mut stdin, rating_db),
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

fn run_sized<const N: usize>(config: GameConfig, p1_name: &str, p2_name: &str, rng: &mut SmallRng, stdout: &mut impl Write, stdin: &mut impl BufRead, rating_db: Box<dyn RatingDb>)
where
	[(); N * N]:, {
	let game: GameState<N> = GameState::new(config, rng);

	let p1_display = format!("{p1_name} ({:?})", PlayerId::Cols);
	let p2_display = format!("{p2_name} ({:?})", PlayerId::Rows);

	let p1_manual = algos::is_manual(p1_name);
	let p2_manual = algos::is_manual(p2_name);

	let p1 = algos::resolve::<N>(p1_name);
	let p2 = algos::resolve::<N>(p2_name);
	let mut m = Match::new(game, p1, p2).with_rating_db(rating_db);

	// Show initial board
	let board_str = m.game().board.to_string();
	writeln!(stdout, "{board_str}").unwrap();
	let mut prev_lines = board_str.chars().filter(|&c| c == '\n').count() + 1;

	loop {
		let game = m.game();
		let current_name = match game.turn {
			PlayerId::Cols => &p1_display,
			PlayerId::Rows => &p2_display,
		};
		let is_manual = match game.turn {
			PlayerId::Cols => p1_manual,
			PlayerId::Rows => p2_manual,
		};

		// Clear previous turn's output
		if prev_lines > 0 {
			write!(stdout, "\x1B[{prev_lines}A\x1B[J").unwrap();
			prev_lines = 0;
		}

		let external_move = if is_manual {
			let hand = &game.hands[game.turn as usize];
			Some(read_manual_move(&game.board, hand, current_name, stdout, stdin))
		} else {
			None
		};

		match m.next(external_move) {
			Ok(game) => {
				let board_str = game.board.to_string();
				let output = format!("au tour de {current_name}\n{board_str}");
				writeln!(stdout, "{output}").unwrap();
				prev_lines = output.chars().filter(|&c| c == '\n').count() + 1;
			}
			Err(result) => {
				// Clear last turn display, show final board + results
				if prev_lines > 0 {
					write!(stdout, "\x1B[{prev_lines}A\x1B[J").unwrap();
				}
				writeln!(stdout, "{}", m.game().board).unwrap();
				display_result::<N>(&result, &p1_display, &p2_display, stdout);
				return;
			}
		}
	}
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

	if let Some(ref elo) = result.elo_update {
		let d1 = elo.p1_new - elo.p1_old;
		let d2 = elo.p2_new - elo.p2_old;
		let sign = |d: f64| if d >= 0.0 { "+" } else { "" };
		writeln!(
			stdout,
			"\nElo: {} {:.0} ({}{:.0}) | {} {:.0} ({}{:.0})",
			result.p1_id,
			elo.p1_new,
			sign(d1),
			d1,
			result.p2_id,
			elo.p2_new,
			sign(d2),
			d2
		)
		.unwrap();
	}
}
