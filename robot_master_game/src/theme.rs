use bevy::prelude::Color;

/// Named colors from the Catppuccin Frappé palette.
/// No alpha — transparency lives at the call site via [`Catppuccin::color_a`].
#[allow(unused)]
#[derive(Clone, Copy)]
pub enum Palette {
	Rosewater,
	Flamingo,
	Pink,
	Mauve,
	Red,
	Maroon,
	Peach,
	Yellow,
	Green,
	Teal,
	Sky,
	Sapphire,
	Blue,
	Lavender,
	Text,
	Subtext1,
	Subtext0,
	Overlay2,
	Overlay1,
	Overlay0,
	Surface2,
	Surface1,
	Surface0,
	Base,
	Mantle,
	Crust,
}

/// Catppuccin Frappé flavor.
pub struct Catppuccin;

impl Catppuccin {
	pub const fn color_a(p: Palette, alpha: f32) -> Color {
		let (l, c, h): (f32, f32, f32) = match p {
			Palette::Rosewater => (0.90, 0.03, 32.0),
			Palette::Flamingo => (0.84, 0.06, 18.0),
			Palette::Pink => (0.85, 0.09, 336.0),
			Palette::Mauve => (0.76, 0.11, 312.0),
			Palette::Red => (0.72, 0.12, 19.0),
			Palette::Maroon => (0.76, 0.10, 17.0),
			Palette::Peach => (0.77, 0.11, 48.0),
			Palette::Yellow => (0.84, 0.08, 83.0),
			Palette::Green => (0.81, 0.11, 133.0),
			Palette::Teal => (0.78, 0.07, 185.0),
			Palette::Sky => (0.83, 0.06, 210.0),
			Palette::Sapphire => (0.78, 0.07, 228.0),
			Palette::Blue => (0.74, 0.10, 266.0),
			Palette::Lavender => (0.81, 0.08, 284.0),
			Palette::Text => (0.86, 0.05, 273.0),
			Palette::Subtext1 => (0.81, 0.05, 273.0),
			Palette::Subtext0 => (0.75, 0.05, 274.0),
			Palette::Overlay2 => (0.70, 0.05, 274.0),
			Palette::Overlay1 => (0.64, 0.04, 273.0),
			Palette::Overlay0 => (0.58, 0.04, 275.0),
			Palette::Surface2 => (0.52, 0.04, 274.0),
			Palette::Surface1 => (0.46, 0.04, 273.0),
			Palette::Surface0 => (0.39, 0.03, 276.0),
			Palette::Base => (0.33, 0.03, 275.0),
			Palette::Mantle => (0.30, 0.03, 276.0),
			Palette::Crust => (0.27, 0.03, 275.0),
		};
		Color::oklcha(l, c, h, alpha)
	}

	pub const fn color(p: Palette) -> Color {
		Self::color_a(p, 1.0)
	}
}

pub mod bg {
	use super::{Catppuccin, Color, Palette};
	pub const DARK: Color = Catppuccin::color(Palette::Base);
	pub const RESULT: Color = Catppuccin::color(Palette::Mantle);
}

pub mod btn {
	use super::{Catppuccin, Color, Palette};
	pub const NORMAL: Color = Catppuccin::color_a(Palette::Surface1, 0.8);
	pub const HOVERED: Color = Catppuccin::color_a(Palette::Surface2, 0.9);
	pub const PRESSED: Color = Catppuccin::color(Palette::Green);
	pub const START: Color = Catppuccin::color(Palette::Teal);
	pub const PLAY_AGAIN: Color = Catppuccin::color(Palette::Teal);
}

pub mod cell {
	use super::{Catppuccin, Color, Palette};
	pub const OCCUPIED: Color = Catppuccin::color(Palette::Surface1);
	pub const EMPTY: Color = Catppuccin::color_a(Palette::Surface0, 0.4);
	pub const HIGHLIGHT: Color = Catppuccin::color_a(Palette::Green, 0.5);
}

pub mod hand {
	use super::{Catppuccin, Color, Palette};
	pub const CARD: Color = Catppuccin::color_a(Palette::Surface1, 0.7);
	pub const CARD_EMPTY: Color = Catppuccin::color_a(Palette::Surface0, 0.3);
	pub const CARD_SELECTED: Color = Catppuccin::color(Palette::Yellow);
	pub const CARD_HOVER: Color = Catppuccin::color_a(Palette::Surface2, 0.9);
	pub const CARD_OPPONENT: Color = Catppuccin::color_a(Palette::Surface0, 0.5);
}

pub mod text {
	use super::{Catppuccin, Color, Palette};
	pub const PRIMARY: Color = Catppuccin::color(Palette::Text);
	pub const TITLE: Color = Catppuccin::color(Palette::Yellow);
	pub const P1: Color = Catppuccin::color(Palette::Blue);
	pub const P2: Color = Catppuccin::color(Palette::Red);
	pub const MUTED: Color = Catppuccin::color(Palette::Overlay1);
	pub const DANGER: Color = Catppuccin::color(Palette::Maroon);
	pub const SELECTION: Color = Catppuccin::color(Palette::Yellow);
	pub const GAME_OVER: Color = Catppuccin::color(Palette::Red);
	pub const ELO: Color = Catppuccin::color(Palette::Subtext1);
}

/// define all layers right here, which allows to not worry about including gaps in used z-indexes
pub mod layer {
	/// Settings modal overlay.
	pub const MODAL: i32 = 1;
	/// Search modal, spawned on top of the settings modal.
	pub const MODAL_SEARCH: i32 = 2;
}
