#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

mod gameplay;
mod menu;
mod result;
mod theme;

use bevy::{asset::AssetMetaCheck, ecs::message::MessageWriter, prelude::*};
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
}

#[cfg(not(target_arch = "wasm32"))]
pub fn create_app(asset_dir: &str, size: BoardSize, p1: PlayerKind, p2: PlayerKind) -> App {
	let mut app = App::new();
	app.insert_resource(InitialPlayers { p1, p2, size });
	configure_app(&mut app, asset_dir.to_string());
	app
}
#[cfg(target_arch = "wasm32")]
pub fn create_app() -> App {
	let mut app = App::new();
	app.insert_resource(InitialPlayers {
		p1: PlayerKind::Manual { name: "Player".into() },
		p2: PlayerKind::Random,
		size: BoardSize::DEFAULT,
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

	app.init_resource::<PressedChars>()
		.init_state::<AppState>()
		.add_systems(Startup, setup)
		.add_systems(Update, (update_pressed_chars, handle_exit).chain())
		.add_plugins((menu::MenuPlugin, gameplay::GameplayPlugin, result::ResultPlugin));
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
	commands.spawn(Camera2d);
	commands.spawn(AudioPlayer::new(asset_server.load("music/robotic_city_v2.ogg")));
}

fn handle_exit(pressed_chars: Res<PressedChars>, keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>, mut colon: Local<bool>) {
	let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
	if ctrl && keys.just_pressed(KeyCode::KeyC) {
		exit.write(AppExit::Success);
	}
	if pressed_chars.just_pressed.contains(&':') {
		*colon = true;
	} else if *colon && pressed_chars.just_pressed.contains(&'q') {
		exit.write(AppExit::Success);
	} else if !pressed_chars.just_pressed.is_empty() && !pressed_chars.just_pressed.contains(&':') {
		*colon = false;
	}
}
