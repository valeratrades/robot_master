#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

mod gameplay;
mod menu;
mod result;
mod theme;

use bevy::{asset::AssetMetaCheck, ecs::message::MessageWriter, prelude::*};
use robot_master_core::cards::CardValue;
use ustr::{Ustr, ustr};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum AppState {
	#[default]
	Menu,
	Playing,
	Result,
}

#[derive(Clone, Debug)]
pub enum PlayerKind {
	Manual { name: String },
	Random,
	Greedy,
	Sadist,
}
impl PlayerKind {
	fn id(&self) -> Ustr {
		ustr(&self.to_string().to_lowercase())
	}

	pub fn from_name(name: &str) -> Self {
		match name {
			"m" | "manual" => PlayerKind::Manual { name: "Player".into() },
			"r" | "random" => PlayerKind::Random,
			"g" | "greedy" => PlayerKind::Greedy,
			"s" | "sadist" => PlayerKind::Sadist,
			other => panic!("unknown player: {other:?}"),
		}
	}
}

/// CLI-provided player selection, consumed by gameplay setup.
#[derive(Clone, Debug, Resource)]
pub struct InitialPlayers {
	pub p1: String,
	pub p2: String,
}

impl std::fmt::Display for PlayerKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PlayerKind::Manual { name } => f.write_str(name),
			PlayerKind::Random => f.write_str("Random"),
			PlayerKind::Greedy => f.write_str("Greedy"),
			PlayerKind::Sadist => f.write_str("Sadist"),
		}
	}
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_app(asset_dir: &str, p1: &str, p2: &str) -> App {
	let mut app = App::new();
	app.insert_resource(InitialPlayers {
		p1: p1.to_string(),
		p2: p2.to_string(),
	});
	configure_app(&mut app, asset_dir.to_string());
	app
}
#[cfg(target_arch = "wasm32")]
pub fn create_app() -> App {
	let mut app = App::new();
	app.insert_resource(InitialPlayers {
		p1: "manual".to_string(),
		p2: "random".to_string(),
	});
	configure_app(&mut app, String::new());
	app
}

/// Shared texture handles, loaded once at startup.
#[derive(Resource)]
struct Textures {
	card_faces: [Handle<Image>; 6],
}
impl Textures {
	fn card_face(&self, v: CardValue) -> Handle<Image> {
		self.card_faces[v.0 as usize].clone()
	}
}

fn configure_app(app: &mut App, file_path: String) {
	app.add_plugins(
		DefaultPlugins
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
		let card_faces = std::array::from_fn(|i| asset_server.load(format!("cards/card_{i}.png")));
		app.insert_resource(Textures { card_faces });
	}

	app.init_state::<AppState>()
		.add_systems(Startup, setup)
		.add_systems(Update, handle_exit)
		.add_plugins((menu::MenuPlugin, gameplay::GameplayPlugin, result::ResultPlugin));
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
	commands.spawn(Camera2d);
	commands.spawn(AudioPlayer::new(asset_server.load("music/robotic_city_v2.ogg")));
}

fn handle_exit(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
	if keys.just_pressed(KeyCode::Escape) {
		exit.write(AppExit::Success);
	}
}
