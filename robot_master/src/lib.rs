#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

pub mod arena;
pub mod config;
pub mod train;
pub mod tui;

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
#[pymodule]
fn robot_master(m: &Bound<'_, PyModule>) -> PyResult<()> {
	m.add_function(wrap_pyfunction!(robot_master_core::python::creer_plateau, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::cases_libres, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::plateau_to_string, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::cases_voisines, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::emplacement_jouable, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::place_carte, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::new_pile_cartes, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::distribution_cartes, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::liste_to_dico, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::init_dico_cartes, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::colonne_to_dico, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::score_ligne_py, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::score_joueuse, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::victoire_py, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::random_move_py, m)?)?;
	m.add_function(wrap_pyfunction!(robot_master_core::python::display_diff_py, m)?)?;
	m.add_function(wrap_pyfunction!(greedy_move_py, m)?)?;
	m.add_function(wrap_pyfunction!(sadist_move_py, m)?)?;
	m.add_function(wrap_pyfunction!(rollout_move_py, m)?)?;
	Ok(())
}

// Board<N> is const-generic: Bot::choose_move needs N known at compile time.
// This macro stamps out the 4-way match (5/7/9/11) so we don't copy-paste it per algorithm.
#[cfg(feature = "python")]
macro_rules! algo_move_dispatch {
	($plateau:expr, $dico_main:expr, $joueuse_active:expr, $player:expr) => {{
		use pyo3::exceptions::PyValueError;
		use robot_master_arena::player::Bot;
		use robot_master_core::{
			cards::Hand,
			game::{GameState, Player},
			python::board_from_plateau,
		};

		let n = $plateau.len();
		let turn = if $joueuse_active % 2 == 0 { Player::A } else { Player::B };

		macro_rules! go {
			($N: literal) => {{
				let hand = Hand::<$N>::from(&$dico_main);
				let board = board_from_plateau::<$N>(&$plateau)?;
				let hands = match turn {
					Player::A => [hand, Hand::default()],
					Player::B => [Hand::default(), hand],
				};
				let state = GameState::from_parts(board, hands, turn);
				let m = $player.choose_move(&state);
				Ok((m.card.0, m.pos.row, m.pos.col))
			}};
		}
		match robot_master_arena::BoardSize::try_from(n as u8) {
			Ok(robot_master_arena::BoardSize::Five) => go!(5),
			Ok(robot_master_arena::BoardSize::Seven) => go!(7),
			Ok(robot_master_arena::BoardSize::Nine) => go!(9),
			Ok(robot_master_arena::BoardSize::Eleven) => go!(11),
			Err(e) => Err(PyValueError::new_err(e)),
		}
	}};
}

#[cfg(feature = "python")]
#[pyfunction]
fn greedy_move_py(plateau: Vec<Vec<Option<u8>>>, dico_main: std::collections::HashMap<u8, u8>, joueuse_active: u8) -> PyResult<(u8, u8, u8)> {
	algo_move_dispatch!(plateau, dico_main, joueuse_active, robot_master_arena::algos::greedy_min::GreedyForScore {})
}

#[cfg(feature = "python")]
#[pyfunction]
fn sadist_move_py(plateau: Vec<Vec<Option<u8>>>, dico_main: std::collections::HashMap<u8, u8>, joueuse_active: u8) -> PyResult<(u8, u8, u8)> {
	algo_move_dispatch!(plateau, dico_main, joueuse_active, robot_master_arena::algos::sadist::Sadist {})
}

#[cfg(feature = "python")]
#[pyfunction]
fn rollout_move_py(plateau: Vec<Vec<Option<u8>>>, dico_main: std::collections::HashMap<u8, u8>, joueuse_active: u8, sims: u32) -> PyResult<(u8, u8, u8)> {
	use robot_master_train::mcts::{RolloutEval, VanillaMcts};
	algo_move_dispatch!(
		plateau,
		dico_main,
		joueuse_active,
		VanillaMcts::new(RolloutEval::new(robot_master_arena::algos::Rollout {}), sims)
	)
}
