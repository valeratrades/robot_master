#![feature(default_field_values)]
#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

pub mod algos;
pub mod config;
pub mod db;
pub mod match_;
pub mod player;
pub mod rating;
pub mod tournament;

use std::{fmt, str::FromStr};

/// Supported board dimensions.
///
/// Each variant's numeric value is obtainable via `Into<u8>` / `From<BoardSize>`.
#[derive(Clone, Copy, Debug, strum::EnumIter, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoardSize {
	Five,
	Seven,
	Nine,
	Eleven,
}

impl BoardSize {
	pub const DEFAULT: Self = BoardSize::Five;
}

impl From<BoardSize> for u8 {
	fn from(s: BoardSize) -> u8 {
		match s {
			BoardSize::Five => 5,
			BoardSize::Seven => 7,
			BoardSize::Nine => 9,
			BoardSize::Eleven => 11,
		}
	}
}

impl TryFrom<u8> for BoardSize {
	type Error = String;

	fn try_from(n: u8) -> Result<Self, Self::Error> {
		match n {
			5 => Ok(BoardSize::Five),
			7 => Ok(BoardSize::Seven),
			9 => Ok(BoardSize::Nine),
			11 => Ok(BoardSize::Eleven),
			_ => Err(format!("unsupported board size {n}")),
		}
	}
}

impl fmt::Display for BoardSize {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", u8::from(*self))
	}
}

impl FromStr for BoardSize {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let n: u8 = s.parse().map_err(|_| format!("invalid board size: {s}"))?;
		BoardSize::try_from(n)
	}
}
