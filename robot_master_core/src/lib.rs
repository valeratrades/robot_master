#![feature(default_field_values)]
#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

pub mod board;
pub mod cards;
pub mod game;
pub mod scoring;

#[cfg(feature = "python")]
pub mod python;
