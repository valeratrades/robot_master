#![feature(default_field_values)]
#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

mod gameplay;
mod menu;
mod result;
mod theme;

use bevy::{asset::AssetMetaCheck, prelude::*};
use robot_master_arena::{BoardSize, algos::PlayerKind};
use robot_master_core::cards::CardValue;
use v_utils::bevy::{PressedChars, update_pressed_chars};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum AppState {
	#[default]
	Menu,
	Playing,
	Result,
}

/// CLI-resolved game setup, consumed by gameplay.
#[derive(Clone, Debug, Resource)]
pub struct InitialPlayers {
	pub p1: PlayerKind,
	pub p2: PlayerKind,
	pub size: BoardSize,
	pub hide: bool,
	pub models_dir: std::path::PathBuf,
	pub eval_model: Option<PlayerKind>,
	/// When true the match result is not committed to the ratings DB.
	pub no_priors: bool,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_app(asset_dir: &str, size: BoardSize, hide: bool, p1: PlayerKind, p2: PlayerKind, sound: bool, models_dir: std::path::PathBuf) -> App {
	let mut app = App::default();
	app.insert_resource(InitialPlayers {
		p1,
		p2,
		size,
		hide,
		models_dir,
		eval_model: None,
		no_priors: false,
	});
	app.insert_resource(SoundEnabled(sound));
	configure_app(&mut app, asset_dir.to_string());
	app
}
#[cfg(target_arch = "wasm32")]
pub fn create_app() -> App {
	use robot_master_arena::{
		algos::{InnerKind, RandomPlayer},
		player::ManualPlayer,
	};

	let mut app = App::default();
	//HACK: don't really like this
	app.insert_resource(InitialPlayers {
		p1: PlayerKind {
			inner: InnerKind::ManualPlayer(ManualPlayer::default()),
			sims: None,
			constrain_sizes: None,
			constrain_hide: None,
		},
		p2: PlayerKind {
			inner: InnerKind::RandomPlayer(RandomPlayer::default()),
			sims: None,
			constrain_sizes: None,
			constrain_hide: None,
		},
		size: BoardSize::DEFAULT,
		hide: false,
		models_dir: std::path::PathBuf::default(),
		eval_model: None,
		no_priors: false,
	});
	app.insert_resource(SoundEnabled(true));
	configure_app(&mut app, String::default());
	app
}
#[derive(Clone, Copy, Debug, Resource)]
struct SoundEnabled(bool);

/// Shared texture handles, loaded once at startup.
#[derive(Resource)]
struct Textures {
	card_faces: Vec<Handle<Image>>,
	card_back: Handle<Image>,
}
impl Textures {
	fn card_face(&self, v: CardValue) -> Handle<Image> {
		self.card_faces.get(v.0 as usize).unwrap_or(&self.card_back).clone()
	}
}

/// Symbols Nerd Font Mono handle, used for icon glyphs (e.g. trash icon in settings).
#[derive(Resource)]
pub(crate) struct NerdFont(pub Handle<Font>);

/// Noto Sans Symbols 2 handle, used for misc Unicode symbols (e.g. ⏎ U+23CE in shortcut hints).
#[derive(Resource)]
pub(crate) struct NotoSymbolsFont(pub Handle<Font>);

fn configure_app(app: &mut App, file_path: String) {
	app.add_plugins(
		DefaultPlugins
			.build()
			.disable::<bevy::app::TerminalCtrlCHandlerPlugin>()
			.set(AssetPlugin {
				meta_check: AssetMetaCheck::Never,
				file_path,
				..default()
			})
			.set(ImagePlugin::default_nearest())
			.set(WindowPlugin {
				primary_window: Some(Window {
					title: "Robot Master".to_string(),
					resolution: (1280u32, 720u32).into(),
					#[cfg(target_arch = "wasm32")]
					canvas: Some("#bevy-canvas".to_string()),
					#[cfg(target_arch = "wasm32")]
					fit_canvas_to_parent: true,
					#[cfg(target_arch = "wasm32")]
					prevent_default_event_handling: true,
					..default()
				}),
				..default()
			}),
	);

	// Insert texture resources eagerly so they're available for OnEnter(Menu) on the first frame.
	{
		let asset_server = app.world().resource::<AssetServer>().clone();
		let card_back = asset_server.load("cards/card_back.png");
		let card_faces = (0..6).map(|i| asset_server.load(format!("cards/card_{i}.png"))).collect();
		app.insert_resource(Textures { card_faces, card_back });
		let nerd_font = asset_server.load("fonts/SymbolsNerdFontMono-Regular.ttf");
		app.insert_resource(NerdFont(nerd_font));
		let noto_symbols = asset_server.load("fonts/NotoSansSymbols2-Regular.otf");
		app.insert_resource(NotoSymbolsFont(noto_symbols));
	}

	app.init_resource::<PressedChars>()
		.init_state::<AppState>()
		.add_systems(Startup, setup)
		.add_systems(Update, update_pressed_chars)
		.add_plugins((menu::MenuPlugin, gameplay::GameplayPlugin, result::ResultPlugin));
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, sound: Res<SoundEnabled>) {
	commands.spawn(Camera2d);
	if sound.0 {
		commands.spawn(AudioPlayer::new(asset_server.load("music/robotic_city_v2.ogg")));
	}
}
