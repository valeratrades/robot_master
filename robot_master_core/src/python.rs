use std::collections::HashMap;

use pyo3::{exceptions::PyValueError, prelude::*, types::PyAny};
use rand::{
	rngs::SmallRng,
	seq::{IteratorRandom, SliceRandom},
};

use crate::{
	board::{Board, Pos},
	cards::{CardValue, Hand},
	game::{GameConfig, Move},
};

// ---------------------------------------------------------------------------
// Conversion helpers (standard trait impls, no helper fns)
// ---------------------------------------------------------------------------

impl<const N: usize> From<&HashMap<u8, u8>> for Hand<N>
where
	[(); N + 1]:,
{
	fn from(m: &HashMap<u8, u8>) -> Self {
		let mut hand = Hand::default();
		for (&v, &c) in m {
			if v as usize <= N {
				for _ in 0..c {
					hand.put(CardValue(v));
				}
			}
		}
		hand
	}
}

/// Create a taille×taille grid of None. Returns None if taille is even or <= 0.
#[pyfunction]
pub fn creer_plateau(taille: i64) -> Option<Vec<Vec<Option<u8>>>> {
	if taille <= 0 || taille % 2 == 0 {
		return None;
	}
	let n = taille as usize;
	Some(vec![vec![None; n]; n])
}
/// Return list of empty cells as (row, col) tuples.
#[pyfunction]
pub fn cases_libres(plateau: Vec<Vec<Option<u8>>>) -> Vec<(usize, usize)> {
	let mut out = Vec::default();
	for (r, row) in plateau.iter().enumerate() {
		for (c, cell) in row.iter().enumerate() {
			if cell.is_none() {
				out.push((r, c));
			}
		}
	}
	out
}
/// Render board as the canonical ASCII string used by tests.
#[pyfunction]
#[pyo3(signature = (plateau, vide = "   "))]
pub fn plateau_to_string(plateau: Vec<Vec<Option<u8>>>, vide: &str) -> String {
	let n = plateau.len();
	let bar: String = "-".repeat(9 + 4 * n);
	let mut lines = Vec::default();
	lines.push(bar.clone());
	// header row - matches Python: " " * 10 + "".join(f"{j}   " for j in range(n)).rstrip()
	let cols: String = (0..n).map(|c| format!("{c}   ")).collect();
	let header = format!("          {}", cols.trim_end());
	lines.push(header);
	lines.push(bar.clone());
	for (r, row) in plateau.iter().enumerate() {
		let cells: String = row
			.iter()
			.map(|cell| match cell {
				Some(v) => format!(" {v} |"),
				None => format!("{vide}|"),
			})
			.collect();
		lines.push(format!("({r},_)   |{cells}"));
	}
	lines.push(bar);
	lines.join("\n")
}
/// Return orthogonal neighbours of (posL, posC) that are within bounds.
#[pyfunction]
pub fn cases_voisines(plateau: Vec<Vec<Option<u8>>>, pos_l: i64, pos_c: i64) -> Vec<(usize, usize)> {
	let n = plateau.len() as i64;
	if pos_l < 0 || pos_l >= n || pos_c < 0 || pos_c >= n {
		return vec![];
	}
	let mut out = Vec::default();
	for (dr, dc) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
		let r = pos_l + dr;
		let c = pos_c + dc;
		if r >= 0 && r < n && c >= 0 && c < n {
			out.push((r as usize, c as usize));
		}
	}
	out
}
/// True if cell is empty and has at least one occupied neighbour.
#[pyfunction]
pub fn emplacement_jouable(plateau: Vec<Vec<Option<u8>>>, pos_l: i64, pos_c: i64) -> bool {
	let n = plateau.len() as i64;
	if pos_l < 0 || pos_l >= n || pos_c < 0 || pos_c >= n {
		return false;
	}
	let (r, c) = (pos_l as usize, pos_c as usize);
	if plateau[r][c].is_some() {
		return false;
	}
	for (nr, nc) in cases_voisines(plateau.clone(), pos_l, pos_c) {
		if plateau[nr][nc].is_some() {
			return true;
		}
	}
	false
}
/// Place carte at (pos_l, pos_c) if the position is playable. Returns updated plateau.
/// Python callers mutate in-place; we return the same list (Python owns it, we just clone in/out).
#[pyfunction]
pub fn place_carte(mut plateau: Vec<Vec<Option<u8>>>, pos_l: i64, pos_c: i64, carte: u8) -> Vec<Vec<Option<u8>>> {
	if emplacement_jouable(plateau.clone(), pos_l, pos_c) {
		plateau[pos_l as usize][pos_c as usize] = Some(carte);
	}
	plateau
}
/// Create a shuffled deck.
/// Accepts either `maxC`/`nbC` (direct card parameters) or `taille` (board side length).
/// With `maxC`/`nbC`: values 0..=maxC, nbC copies each.
/// With `taille` (default 5): values 0..=(taille), (taille+1) copies each.
#[pyfunction]
#[pyo3(signature = (dico_options = None))]
pub fn new_pile_cartes(dico_options: Option<HashMap<String, i64>>) -> Vec<u8> {
	let opts = dico_options.unwrap_or_default();
	let mut rng: SmallRng = rand::make_rng();
	if let (Some(&max_c), Some(&nb_c)) = (opts.get("maxC"), opts.get("nbC")) {
		let mut deck: Vec<u8> = (0..=max_c as u8).flat_map(|v| std::iter::repeat_n(v, nb_c as usize)).collect();
		deck.shuffle(&mut rng);
		deck
	} else {
		let n = opts.get("taille").copied().unwrap_or(5) as usize;
		crate::cards::new_deck(n, &mut rng).into_iter().map(|c| c.0).collect()
	}
}
/// Distribute cards: returns [center_card, hand1_list, hand2_list, ...].
#[pyfunction]
#[pyo3(signature = (pile_cartes, dico_options = None))]
pub fn distribution_cartes(pile_cartes: Vec<u8>, dico_options: Option<HashMap<String, i64>>) -> Vec<Py<PyAny>> {
	Python::try_attach(|py| {
		let opts = dico_options.unwrap_or_default();
		let nb_j = opts.get("nbJ").copied().unwrap_or(2) as usize;
		//HACK: apparently in IA_test.py they try to force different number of cards. So uhh, gotta support that bullshit
		let cartes_distrib = if let Some(&cd) = opts.get("cartes_distrib") {
			cd as usize
		} else {
			let taille = opts.get("taille").copied().unwrap_or(5) as usize;
			(taille * taille - 1) / 2
		};

		let mut result: Vec<Py<PyAny>> = Vec::default();
		// first element: center card (scalar int)
		result.push(pile_cartes[0].into_pyobject(py).unwrap().into_any().unbind());
		// then one hand list per player (as list[int], not bytes)
		let mut idx = 1usize;
		for _ in 0..nb_j {
			let hand: Vec<i64> = pile_cartes[idx..idx + cartes_distrib].iter().map(|&v| v as i64).collect();
			result.push(hand.into_pyobject(py).unwrap().into_any().unbind());
			idx += cartes_distrib;
		}
		result
	})
	.expect("Python interpreter not available")
}
/// Convert card list to frequency dict {card_value: count}, all values 0..=maxC present.
#[pyfunction]
#[pyo3(signature = (cards, dico_options = None))]
pub fn liste_to_dico(cards: Vec<u8>, dico_options: Option<HashMap<String, i64>>) -> HashMap<u8, u64> {
	let opts = dico_options.unwrap_or_default();
	let max_c = opts.get("maxC").copied().unwrap_or(5) as u8;
	let mut dico: HashMap<u8, u64> = (0..=max_c).map(|v| (v, 0)).collect();
	for c in cards {
		*dico.entry(c).or_insert(0) += 1;
	}
	dico
}
/// Create {0..=maxC: 0} dict.
#[pyfunction]
#[pyo3(signature = (dico_options = None))]
pub fn init_dico_cartes(dico_options: Option<HashMap<String, i64>>) -> HashMap<u8, u64> {
	let opts = dico_options.unwrap_or_default();
	let max_c = opts.get("maxC").copied().unwrap_or(5) as u8;
	(0..=max_c).map(|v| (v, 0)).collect()
}
/// Extract row (joueuse_active odd) or column (even) i as frequency dict.
#[pyfunction]
#[pyo3(signature = (plateau, joueuse_active, i, dico_options = None))]
pub fn colonne_to_dico(plateau: Vec<Vec<Option<u8>>>, joueuse_active: u8, i: usize, dico_options: Option<HashMap<String, i64>>) -> HashMap<u8, u64> {
	let opts = dico_options.unwrap_or_default();
	let max_c = opts.get("maxC").copied().unwrap_or(5) as u8;
	let mut dico: HashMap<u8, u64> = (0..=max_c).map(|v| (v, 0)).collect();
	let n = plateau.len();
	for j in 0..n {
		let cell = if joueuse_active % 2 == 1 { plateau[i][j] } else { plateau[j][i] };
		if let Some(v) = cell {
			*dico.entry(v).or_insert(0) += 1;
		}
	}
	dico
}
/// Score a line dict: 1→v, 2→10v, 3+→100.
#[pyfunction]
pub fn score_ligne_py(dico_ligne: HashMap<u8, u64>) -> u64 {
	let mut s = 0u64;
	for (v, c) in &dico_ligne {
		s += match c {
			0 => 0,
			1 => *v as u64,
			2 => 10 * *v as u64,
			_ => 100,
		};
	}
	s
}
/// (min_score, line_idx) for a player.
#[pyfunction]
#[pyo3(signature = (plateau, joueuse_active, dico_options = None))]
pub fn score_joueuse(plateau: Vec<Vec<Option<u8>>>, joueuse_active: u8, dico_options: Option<HashMap<String, i64>>) -> PyResult<(u64, usize)> {
	let opts = dico_options.unwrap_or_default();
	let n = plateau.len();
	let mut best_score = u64::MAX;
	let mut best_idx = 0;
	for i in 0..n {
		let d = colonne_to_dico(plateau.clone(), joueuse_active, i, Some(opts.clone()));
		let s = score_ligne_py(d);
		if s < best_score {
			best_score = s;
			best_idx = i;
		}
	}
	Ok((best_score, best_idx))
}
/// (score_p0, idx_p0, score_p1, idx_p1)
#[pyfunction]
#[pyo3(signature = (plateau, dico_options = None))]
pub fn victoire_py(plateau: Vec<Vec<Option<u8>>>, dico_options: Option<HashMap<String, i64>>) -> PyResult<(u64, usize, u64, usize)> {
	let (s0, i0) = score_joueuse(plateau.clone(), 0, dico_options.clone())?;
	let (s1, i1) = score_joueuse(plateau, 1, dico_options)?;
	Ok((s0, i0, s1, i1))
}
#[pyfunction]
pub fn random_move_py(plateau: Vec<Vec<Option<u8>>>, dico_main: HashMap<u8, u8>, dico_options: HashMap<String, i64>) -> PyResult<(u8, u8, u8)> {
	let config = config_from_options(&dico_options);

	let m: Option<Move> = match config.size {
		5 => {
			let hand = Hand::<5>::from(&dico_main);
			let board = board_from_plateau::<5>(&plateau)?;
			let mut rng: SmallRng = rand::make_rng();
			board.valid_placements().flat_map(|pos| hand.iter_playable().map(move |card| Move { pos, card })).choose(&mut rng)
		}
		7 => {
			let hand = Hand::<7>::from(&dico_main);
			let board = board_from_plateau::<7>(&plateau)?;
			let mut rng: SmallRng = rand::make_rng();
			board.valid_placements().flat_map(|pos| hand.iter_playable().map(move |card| Move { pos, card })).choose(&mut rng)
		}
		9 => {
			let hand = Hand::<9>::from(&dico_main);
			let board = board_from_plateau::<9>(&plateau)?;
			let mut rng: SmallRng = rand::make_rng();
			board.valid_placements().flat_map(|pos| hand.iter_playable().map(move |card| Move { pos, card })).choose(&mut rng)
		}
		11 => {
			let hand = Hand::<11>::from(&dico_main);
			let board = board_from_plateau::<11>(&plateau)?;
			let mut rng: SmallRng = rand::make_rng();
			board.valid_placements().flat_map(|pos| hand.iter_playable().map(move |card| Move { pos, card })).choose(&mut rng)
		}
		n => return Err(PyValueError::new_err(format!("unsupported board size {n}"))),
	};

	let m = m.ok_or_else(|| PyValueError::new_err("no valid moves available"))?;
	Ok((m.card.0, m.pos.row, m.pos.col))
}
pub fn config_from_options(opts: &HashMap<String, i64>) -> GameConfig {
	GameConfig {
		size: opts.get("taille").copied().unwrap_or(5) as u8,
		hide: false,
	}
}

/// Convert Python `plateau` (list[list[int|None]]) into a Board.
pub fn board_from_plateau<const N: usize>(plateau: &[Vec<Option<u8>>]) -> PyResult<Board<N>>
where
	[(); N * N]:, {
	if plateau.len() != N {
		return Err(PyValueError::new_err(format!("expected {N}x{N} board, got {}x?", plateau.len())));
	}
	let mut board = Board::default();
	for (row, row_data) in plateau.iter().enumerate() {
		if row_data.len() != N {
			return Err(PyValueError::new_err(format!("row {row} has len {}, expected {N}", row_data.len())));
		}
		for (col, &cell) in row_data.iter().enumerate() {
			if let Some(v) = cell {
				board.set(Pos { row: row as u8, col: col as u8 }, v);
			}
		}
	}
	Ok(board)
}

/// Diff display between two board states. New cells shown as `+v`.
#[pyfunction]
pub fn display_diff_py(plateau: Vec<Vec<Option<u8>>>, prev: Vec<Vec<Option<u8>>>) -> PyResult<String> {
	let n = plateau.len();
	match n {
		5 => Ok(board_from_plateau::<5>(&plateau)?.display_diff(&board_from_plateau::<5>(&prev)?)),
		7 => Ok(board_from_plateau::<7>(&plateau)?.display_diff(&board_from_plateau::<7>(&prev)?)),
		9 => Ok(board_from_plateau::<9>(&plateau)?.display_diff(&board_from_plateau::<9>(&prev)?)),
		11 => Ok(board_from_plateau::<11>(&plateau)?.display_diff(&board_from_plateau::<11>(&prev)?)),
		_ => Err(PyValueError::new_err(format!("unsupported board size {n}"))),
	}
}

// ---------------------------------------------------------------------------
// a_plateau equivalents
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// b_gestionCartes equivalents
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// d_score equivalents
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// random move (existing)
// ---------------------------------------------------------------------------
