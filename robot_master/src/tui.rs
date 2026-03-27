use std::{
	fmt,
	io::{self, BufRead, Write},
};

use rand::{SeedableRng, rngs::SmallRng, seq::IteratorRandom};
use robot_master_core::{
	board::{Board, EMPTY, Pos},
	cards::{CardValue, Hand},
	game::{GameConfig, GameState, Move},
	scoring::victoire,
};

pub fn run(config: GameConfig, manual: [bool; 2], names: [&str; 2]) {
	let stdout_handle = io::stdout();
	let mut stdout = stdout_handle.lock();
	let stdin_handle = io::stdin();
	let mut stdin = stdin_handle.lock();
	let mut rng = SmallRng::from_os_rng();

	match config.size {
		5 => run_sized::<5>(config, manual, names, &mut rng, &mut stdout, &mut stdin),
		7 => run_sized::<7>(config, manual, names, &mut rng, &mut stdout, &mut stdin),
		9 => run_sized::<9>(config, manual, names, &mut rng, &mut stdout, &mut stdin),
		11 => run_sized::<11>(config, manual, names, &mut rng, &mut stdout, &mut stdin),
		n => panic!("unsupported board size {n}"),
	}
}
struct BoardDisplay<'a, const N: usize>(&'a Board<N>)
where
	[(); N * N]:;

impl<const N: usize> fmt::Display for BoardDisplay<'_, N>
where
	[(); N * N]:,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let bar: String = "-".repeat(9 + 4 * N);
		writeln!(f, "{bar}")?;
		write!(f, "          ")?;
		for c in 0..N {
			if c + 1 < N {
				write!(f, "{c}   ")?;
			} else {
				write!(f, "{c}")?;
			}
		}
		writeln!(f)?;
		writeln!(f, "{bar}")?;
		for row in 0..N {
			write!(f, "({row},_)   |")?;
			for col in 0..N {
				let cell = self.0.get(Pos { row: row as u8, col: col as u8 });
				if cell == EMPTY {
					write!(f, "   |")?;
				} else {
					write!(f, " {cell} |")?;
				}
			}
			writeln!(f)?;
		}
		write!(f, "{bar}")?;
		Ok(())
	}
}

fn hand_display(hand: &Hand) -> String {
	let pairs: Vec<String> = (0..=5u8).filter(|&v| hand.count(CardValue(v)) > 0).map(|v| format!("{v}:{}", hand.count(CardValue(v)))).collect();
	format!("{{{}}}", pairs.join(", "))
}

fn read_manual_move<const N: usize>(board: &Board<N>, hand: &Hand, name: &str, stdout: &mut impl Write, stdin: &mut impl BufRead) -> Move
where
	[(); N * N]:, {
	let mut lines_to_erase = 0usize;
	let mut warning: Option<String> = None;
	loop {
		if lines_to_erase > 0 {
			write!(stdout, "\x1B[{lines_to_erase}A\x1B[J").unwrap();
		}
		let board_str = format!("{}", BoardDisplay(board));
		let board_lines = board_str.lines().count();
		write!(stdout, "{board_str}\n").unwrap();
		writeln!(stdout, "{name}, votre main : {}", hand_display(hand)).unwrap();
		let mut prompt_lines = 0usize;
		let base_lines = board_lines + 1; // board + hand line

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

		return Move { pos, card: CardValue(carte) };
	}
}

fn random_move<const N: usize>(state: &GameState<N>, rng: &mut impl rand::Rng) -> Move
where
	[(); N * N]:, {
	state.valid_moves().choose(rng).expect("no valid moves")
}

/// Single turn: clear previous output, get move, redraw board.
fn turn<const N: usize>(state: &mut GameState<N>, names: &[&str; 2], prev_lines: &mut usize, manual: [bool; 2], rng: &mut SmallRng, stdout: &mut impl Write, stdin: &mut impl BufRead)
where
	[(); N * N]:, {
	let player_idx = state.turn as usize;
	let name = names[player_idx];

	// clear previous turn's output
	if *prev_lines > 0 {
		write!(stdout, "\x1B[{}A\x1B[J", *prev_lines).unwrap();
		*prev_lines = 0;
	}

	let m = if manual[player_idx] {
		read_manual_move(&state.board, &state.hands[player_idx], name, stdout, stdin)
	} else {
		random_move(state, rng)
	};

	*state = state.apply_move(m).expect("move was validated but failed");

	// redraw board with turn header
	let board_str = format!("{}", BoardDisplay(&state.board));
	let output = format!("au tour de {name}\n{board_str}");
	writeln!(stdout, "{output}").unwrap();
	*prev_lines = output.lines().count() + 1;
}

fn run_sized<const N: usize>(config: GameConfig, manual: [bool; 2], names: [&str; 2], rng: &mut SmallRng, stdout: &mut impl Write, stdin: &mut impl BufRead)
where
	[(); N * N]:, {
	let mut state: GameState<N> = GameState::new(config, rng);
	// show initial board
	let board_str = format!("{}", BoardDisplay(&state.board));
	writeln!(stdout, "{board_str}").unwrap();
	let mut prev_lines = board_str.lines().count() + 1;

	while !state.is_terminal() {
		turn(&mut state, &names, &mut prev_lines, manual, rng, stdout, stdin);
	}

	// clear last turn display, show final results
	if prev_lines > 0 {
		write!(stdout, "\x1B[{prev_lines}A\x1B[J").unwrap();
	}

	let (s0, i0, s1, i1) = victoire(&state.board);
	let board_str = format!("{}", BoardDisplay(&state.board));

	let verdict = match s0.cmp(&s1) {
		std::cmp::Ordering::Greater => format!("{} wins.", names[0]),
		std::cmp::Ordering::Less => format!("{} wins.", names[1]),
		std::cmp::Ordering::Equal => "draw.".into(),
	};

	writeln!(stdout, "{board_str}\n{}: score {s0} (column {i0})\n{}: score {s1} (row {i1})\n{verdict}", names[0], names[1]).unwrap();
}
