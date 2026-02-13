//! Bevy game module - sprite animation demo

#[cfg(not(target_arch = "wasm32"))]
pub mod config;

use std::time::Duration;

use bevy::{asset::AssetMetaCheck, input::common_conditions::input_just_pressed, prelude::*};
#[cfg(not(target_arch = "wasm32"))]
use {config::LiveSettings, std::sync::Arc, v_utils::utils::exit_on_error};

/// Creates a Bevy App configured for the game.
/// Call `.run()` on it to start.
///
/// `asset_dir`: path to the assets directory, resolved relative to `BEVY_ASSET_ROOT` / `CARGO_MANIFEST_DIR` / exe dir by Bevy.
#[cfg(not(target_arch = "wasm32"))]
pub fn create_app(settings: Arc<LiveSettings>, asset_dir: &str) -> App {
	let mut app = App::new();

	app.insert_resource(Settings(settings));

	configure_app(&mut app, asset_dir);
	app
}
#[cfg(target_arch = "wasm32")]
pub fn create_app() -> App {
	let mut app = App::new();
	configure_app(&mut app);
	app
}
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct Settings(Arc<LiveSettings>);

#[cfg(not(target_arch = "wasm32"))]
fn configure_app(app: &mut App, asset_dir: &str) {
	configure_app_inner(app, asset_dir.to_string());
}
#[cfg(target_arch = "wasm32")]
fn configure_app(app: &mut App) {
	// Leptos serves the `public` assets-dir at root `/`, so wasm uses an empty path.
	configure_app_inner(app, String::new());
}

fn configure_app_inner(app: &mut App, file_path: String) {
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
	)
	.add_systems(Startup, setup)
	.add_systems(Update, execute_animations)
	.add_systems(
		Update,
		(
			trigger_animation::<RightSprite>.run_if(input_just_pressed(KeyCode::ArrowRight)),
			trigger_animation::<LeftSprite>.run_if(input_just_pressed(KeyCode::ArrowLeft)),
		),
	);
}

fn trigger_animation<S: Component>(mut animation: Single<&mut AnimationConfig, With<S>>) {
	animation.frame_timer = AnimationConfig::timer_from_fps(animation.fps);
}

#[derive(Component)]
struct AnimationConfig {
	first_sprite_index: usize,
	last_sprite_index: usize,
	fps: u8,
	frame_timer: Timer,
}

impl AnimationConfig {
	fn new(first: usize, last: usize, fps: u8) -> Self {
		Self {
			first_sprite_index: first,
			last_sprite_index: last,
			fps,
			frame_timer: Self::timer_from_fps(fps),
		}
	}

	fn timer_from_fps(fps: u8) -> Timer {
		Timer::new(Duration::from_secs_f32(1.0 / (fps as f32)), TimerMode::Once)
	}
}

fn execute_animations(time: Res<Time>, mut query: Query<(&mut AnimationConfig, &mut Sprite)>) {
	for (mut config, mut sprite) in &mut query {
		config.frame_timer.tick(time.delta());

		if config.frame_timer.just_finished()
			&& let Some(atlas) = &mut sprite.texture_atlas
		{
			if atlas.index == config.last_sprite_index {
				atlas.index = config.first_sprite_index;
			} else {
				atlas.index += 1;
				config.frame_timer = AnimationConfig::timer_from_fps(config.fps);
			}
		}
	}
}

#[derive(Component)]
struct LeftSprite;

#[derive(Component)]
struct RightSprite;

#[cfg(not(target_arch = "wasm32"))]
fn setup(settings: Res<Settings>, mut commands: Commands, asset_server: Res<AssetServer>, mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>) {
	let config = exit_on_error(settings.0.config());
	let greeting = format!("Hello {}!\nLeft Arrow: Animate Left Sprite\nRight Arrow: Animate Right Sprite", config.example_greet);
	setup_inner(&mut commands, &asset_server, &mut texture_atlas_layouts, &greeting);
}

#[cfg(target_arch = "wasm32")]
fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>) {
	let greeting = "Hello World!\nLeft Arrow: Animate Left Sprite\nRight Arrow: Animate Right Sprite";
	setup_inner(&mut commands, &asset_server, &mut texture_atlas_layouts, greeting);
}

fn setup_inner(commands: &mut Commands, asset_server: &Res<AssetServer>, texture_atlas_layouts: &mut ResMut<Assets<TextureAtlasLayout>>, greeting: &str) {
	commands.spawn(Camera2d);

	commands.spawn((
		Text::new(greeting),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Px(12.0),
			left: Val::Px(12.0),
			..default()
		},
	));

	let texture = asset_server.load("textures/rpg/chars/gabe/gabe-idle-run.png");
	let layout = TextureAtlasLayout::from_grid(UVec2::splat(24), 7, 1, None, None);
	let texture_atlas_layout = texture_atlas_layouts.add(layout);

	let animation_config_1 = AnimationConfig::new(1, 6, 10);
	commands.spawn((
		Sprite {
			image: texture.clone(),
			texture_atlas: Some(TextureAtlas {
				layout: texture_atlas_layout.clone(),
				index: animation_config_1.first_sprite_index,
			}),
			..default()
		},
		Transform::from_scale(Vec3::splat(6.0)).with_translation(Vec3::new(-70.0, 0.0, 0.0)),
		LeftSprite,
		animation_config_1,
	));

	let animation_config_2 = AnimationConfig::new(1, 6, 20);
	commands.spawn((
		Sprite {
			image: texture.clone(),
			texture_atlas: Some(TextureAtlas {
				layout: texture_atlas_layout.clone(),
				index: animation_config_2.first_sprite_index,
			}),
			..Default::default()
		},
		Transform::from_scale(Vec3::splat(6.0)).with_translation(Vec3::new(70.0, 0.0, 0.0)),
		RightSprite,
		animation_config_2,
	));
}
