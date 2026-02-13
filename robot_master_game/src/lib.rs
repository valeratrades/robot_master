#[cfg(not(target_arch = "wasm32"))]
pub mod config;

use std::time::Duration;

use bevy::{asset::AssetMetaCheck, prelude::*};
#[cfg(not(target_arch = "wasm32"))]
use {config::LiveSettings, std::sync::Arc, v_utils::utils::exit_on_error};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlayerId {
	Player1,
	Player2,
}

impl PlayerId {
	pub fn color(self) -> Color {
		match self {
			PlayerId::Player1 => Color::srgb(0.2, 0.6, 1.0),
			PlayerId::Player2 => Color::srgb(1.0, 0.3, 0.3),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			PlayerId::Player1 => "Player 1",
			PlayerId::Player2 => "Player 2",
		}
	}

	pub fn spawn_position(self) -> Vec3 {
		match self {
			PlayerId::Player1 => Vec3::new(-100.0, 0.0, 0.0),
			PlayerId::Player2 => Vec3::new(100.0, 0.0, 0.0),
		}
	}
}

#[derive(Resource)]
pub struct LocalPlayer(pub PlayerId);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct Settings(Arc<LiveSettings>);

#[derive(Component)]
struct Player {
	id: PlayerId,
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

const MOVE_SPEED: f32 = 200.0;

#[cfg(not(target_arch = "wasm32"))]
pub fn create_app(settings: Arc<LiveSettings>, asset_dir: &str, local_player: PlayerId) -> App {
	let mut app = App::new();
	app.insert_resource(Settings(settings));
	app.insert_resource(LocalPlayer(local_player));
	configure_app(&mut app, asset_dir);
	app
}

#[cfg(target_arch = "wasm32")]
pub fn create_app() -> App {
	let mut app = App::new();
	app.insert_resource(LocalPlayer(PlayerId::Player1));
	configure_app(&mut app);
	app
}

#[cfg(not(target_arch = "wasm32"))]
fn configure_app(app: &mut App, asset_dir: &str) {
	configure_app_inner(app, asset_dir.to_string());
}

#[cfg(target_arch = "wasm32")]
fn configure_app(app: &mut App) {
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
	.add_systems(Update, (move_local_player, execute_animations));
}

#[cfg(not(target_arch = "wasm32"))]
fn setup(settings: Res<Settings>, local_player: Res<LocalPlayer>, mut commands: Commands, asset_server: Res<AssetServer>, mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>) {
	let config = exit_on_error(settings.0.config());
	let greeting = format!("Hello {}! You are {}", config.example_greet, local_player.0.name());
	setup_inner(&mut commands, &asset_server, &mut texture_atlas_layouts, &greeting);
}

#[cfg(target_arch = "wasm32")]
fn setup(local_player: Res<LocalPlayer>, mut commands: Commands, asset_server: Res<AssetServer>, mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>) {
	let greeting = format!("Hello World! You are {}", local_player.0.name());
	setup_inner(&mut commands, &asset_server, &mut texture_atlas_layouts, &greeting);
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

	for player_id in [PlayerId::Player1, PlayerId::Player2] {
		let anim = AnimationConfig::new(1, 6, 12);
		let pos = player_id.spawn_position();

		commands
			.spawn((
				Sprite {
					image: texture.clone(),
					texture_atlas: Some(TextureAtlas {
						layout: texture_atlas_layout.clone(),
						index: anim.first_sprite_index,
					}),
					..default()
				},
				Transform::from_scale(Vec3::splat(6.0)).with_translation(pos),
				Player { id: player_id },
				anim,
			))
			.with_children(|parent| {
				parent.spawn((
					Text2d::new(player_id.name()),
					TextColor(player_id.color()),
					TextFont { font_size: 5.0, ..default() },
					Transform::from_translation(Vec3::new(0.0, 16.0, 1.0)),
				));
			});
	}
}

fn move_local_player(time: Res<Time>, keyboard: Res<ButtonInput<KeyCode>>, local_player: Res<LocalPlayer>, mut query: Query<(&Player, &mut Transform, &mut AnimationConfig)>) {
	let mut direction = Vec2::ZERO;
	if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
		direction.y += 1.0;
	}
	if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
		direction.y -= 1.0;
	}
	if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
		direction.x -= 1.0;
	}
	if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
		direction.x += 1.0;
	}

	let moving = direction != Vec2::ZERO;
	if moving {
		direction = direction.normalize();
	}

	for (player, mut transform, mut anim) in &mut query {
		if player.id != local_player.0 {
			continue;
		}
		if moving {
			transform.translation.x += direction.x * MOVE_SPEED * time.delta_secs();
			transform.translation.y += direction.y * MOVE_SPEED * time.delta_secs();
			// keep animation running while moving
			if anim.frame_timer.is_finished() {
				anim.frame_timer = AnimationConfig::timer_from_fps(anim.fps);
			}
		}
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
