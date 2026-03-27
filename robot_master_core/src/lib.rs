#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

pub mod board;
pub mod cards;
pub mod game;
pub mod scoring;

mod python;

use pyo3::prelude::*;

#[pymodule]
fn robot_master_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
	m.add_function(wrap_pyfunction!(python::creer_plateau, m)?)?;
	m.add_function(wrap_pyfunction!(python::cases_libres, m)?)?;
	m.add_function(wrap_pyfunction!(python::plateau_to_string, m)?)?;
	m.add_function(wrap_pyfunction!(python::cases_voisines, m)?)?;
	m.add_function(wrap_pyfunction!(python::emplacement_jouable, m)?)?;
	m.add_function(wrap_pyfunction!(python::place_carte, m)?)?;
	m.add_function(wrap_pyfunction!(python::new_pile_cartes, m)?)?;
	m.add_function(wrap_pyfunction!(python::distribution_cartes, m)?)?;
	m.add_function(wrap_pyfunction!(python::liste_to_dico, m)?)?;
	m.add_function(wrap_pyfunction!(python::init_dico_cartes, m)?)?;
	m.add_function(wrap_pyfunction!(python::colonne_to_dico, m)?)?;
	m.add_function(wrap_pyfunction!(python::score_ligne_py, m)?)?;
	m.add_function(wrap_pyfunction!(python::score_joueuse, m)?)?;
	m.add_function(wrap_pyfunction!(python::victoire_py, m)?)?;
	m.add_function(wrap_pyfunction!(python::random_move_py, m)?)?;
	Ok(())
}
