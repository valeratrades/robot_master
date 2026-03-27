#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

pub mod algos;
pub mod config;
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
	Ok(())
}
