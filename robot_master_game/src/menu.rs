use std::collections::HashMap;

use bevy::prelude::*;
use robot_master_arena::{BoardSize, algos::PlayerKind, rating::Rating};
use strum::IntoEnumIterator;
use ustr::Ustr;

use crate::{AppState, InitialPlayers, theme};

pub struct MenuPlugin;
/// Cached Elo ratings, loaded once per menu entry.
#[derive(Resource)]
struct Ratings(HashMap<Ustr, Rating>);

impl Plugin for MenuPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(OnEnter(AppState::Menu), setup_menu)
			.add_systems(Update, (button_system, dropdown_system, keyboard_shortcuts).run_if(in_state(AppState::Menu)))
			.add_systems(OnExit(AppState::Menu), cleanup_menu);
	}
}

#[derive(Component)]
struct MenuScene;

#[derive(Component)]
struct StartButton;

#[derive(Component)]
struct PlayerButton(usize);

#[derive(Component)]
struct PlayerLabel(usize);

#[derive(Component)]
struct SizeButton;

#[derive(Component)]
struct SizeLabel;

#[derive(Component)]
struct DropdownOption {
	player_idx: usize,
	kind: PlayerKind,
}

#[derive(Component)]
struct SizeDropdownOption(BoardSize);

#[derive(Component)]
struct DropdownPanel;

fn format_player_label(kind: &PlayerKind, ratings: &HashMap<Ustr, Rating>) -> String {
	let name = kind.to_string();
	match ratings.get(&kind.id()) {
		Some(elo) => format!("{name} ({:.0})", elo.rating),
		None => name,
	}
}

fn setup_menu(mut commands: Commands, init: Res<InitialPlayers>, asset_server: Res<AssetServer>) {
	let ratings = load_ratings();
	let p1_kind = init.p1.clone();
	let p2_kind = init.p2.clone();

	// Background image (dimmed)
	commands
		.spawn((
			MenuScene,
			Node {
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				..default()
			},
			ImageNode {
				image: asset_server.load("images/preview.png"),
				color: Color::srgba(1.0, 1.0, 1.0, 0.3),
				..default()
			},
		))
		.with_children(|bg| {
			// Dark overlay
			bg.spawn((
				Node {
					width: Val::Percent(100.0),
					height: Val::Percent(100.0),
					position_type: PositionType::Absolute,
					flex_direction: FlexDirection::Column,
					align_items: AlignItems::Center,
					justify_content: JustifyContent::Center,
					..default()
				},
				BackgroundColor(Color::oklcha(0.0, 0.0, 0.0, 0.55)),
			))
			.with_children(|overlay| {
				// Content panel with squircle corners
				overlay
					.spawn((
						Node {
							flex_direction: FlexDirection::Column,
							align_items: AlignItems::Center,
							justify_content: JustifyContent::Center,
							row_gap: Val::Px(20.0),
							padding: UiRect::axes(Val::Px(60.0), Val::Px(40.0)),
							border_radius: BorderRadius::all(Val::Px(24.0)),
							..default()
						},
						BackgroundColor(Color::oklcha(0.12, 0.02, 260.0, 0.75)),
					))
					.with_children(|panel| {
						panel.spawn((Text::new("ROBOT MASTER"), TextFont { font_size: 64.0, ..default() }, TextColor(theme::TEXT_TITLE)));
						panel.spawn(player_button(0, format_player_label(&p1_kind, &ratings)));
						panel.spawn(player_button(1, format_player_label(&p2_kind, &ratings)));
						panel.spawn(size_button(init.size));
						panel.spawn((
							StartButton,
							Button,
							Node {
								width: Val::Px(200.0),
								height: Val::Px(60.0),
								justify_content: JustifyContent::Center,
								align_items: AlignItems::Center,
								margin: UiRect::top(Val::Px(20.0)),
								border_radius: BorderRadius::all(Val::Px(12.0)),
								..default()
							},
							BackgroundColor(theme::BTN_START),
							children![(Text::new("START"), TextFont { font_size: 36.0, ..default() }, TextColor(theme::TEXT_PRIMARY))],
						));
					});
			});
		});

	commands.insert_resource(Ratings(ratings));
}

fn player_button(idx: usize, display: String) -> impl Bundle {
	let label = match idx {
		0 => "Player 1",
		_ => "Player 2",
	};
	(
		PlayerButton(idx),
		Button,
		Node {
			width: Val::Px(400.0),
			height: Val::Px(50.0),
			justify_content: JustifyContent::SpaceBetween,
			align_items: AlignItems::Center,
			padding: UiRect::horizontal(Val::Px(20.0)),
			border_radius: BorderRadius::all(Val::Px(10.0)),
			..default()
		},
		BackgroundColor(theme::BTN_NORMAL),
		children![
			(Text::new(label), TextFont { font_size: 24.0, ..default() }, TextColor(theme::TEXT_PRIMARY)),
			(PlayerLabel(idx), Text::new(display), TextFont { font_size: 22.0, ..default() }, TextColor(theme::TEXT_LABEL)),
		],
	)
}

fn size_button(size: BoardSize) -> impl Bundle {
	(
		SizeButton,
		Button,
		Node {
			width: Val::Px(400.0),
			height: Val::Px(50.0),
			justify_content: JustifyContent::SpaceBetween,
			align_items: AlignItems::Center,
			padding: UiRect::horizontal(Val::Px(20.0)),
			border_radius: BorderRadius::all(Val::Px(10.0)),
			..default()
		},
		BackgroundColor(theme::BTN_NORMAL),
		children![
			(Text::new("Board Size"), TextFont { font_size: 24.0, ..default() }, TextColor(theme::TEXT_PRIMARY)),
			(
				SizeLabel,
				Text::new(format!("{}x{}", u8::from(size), u8::from(size))),
				TextFont { font_size: 22.0, ..default() },
				TextColor(theme::TEXT_LABEL)
			),
		],
	)
}

#[allow(clippy::type_complexity)]
fn button_system(
	mut interaction_query: Query<
		(
			&Interaction,
			&mut BackgroundColor,
			Option<&StartButton>,
			Option<&PlayerButton>,
			Option<&SizeButton>,
			Option<&DropdownOption>,
			Option<&SizeDropdownOption>,
		),
		Changed<Interaction>,
	>,
	mut next_state: ResMut<NextState<AppState>>,
	mut commands: Commands,
	existing_dropdowns: Query<Entity, With<DropdownPanel>>,
	mut init: ResMut<InitialPlayers>,
	mut label_query: Query<(&PlayerLabel, &mut Text), Without<SizeLabel>>,
	mut size_label: Query<&mut Text, With<SizeLabel>>,
	ratings: Res<Ratings>,
) {
	for (interaction, mut color, start, player_btn, size_btn, dropdown_opt, size_opt) in &mut interaction_query {
		match *interaction {
			Interaction::Pressed => {
				if start.is_some() {
					next_state.set(AppState::Playing);
				} else if let Some(btn) = player_btn {
					let has_dropdown = !existing_dropdowns.is_empty();
					for entity in &existing_dropdowns {
						commands.entity(entity).despawn();
					}
					if !has_dropdown {
						spawn_player_dropdown(&mut commands, btn.0, &ratings.0);
					}
				} else if size_btn.is_some() {
					let has_dropdown = !existing_dropdowns.is_empty();
					for entity in &existing_dropdowns {
						commands.entity(entity).despawn();
					}
					if !has_dropdown {
						spawn_size_dropdown(&mut commands);
					}
				} else if let Some(opt) = dropdown_opt {
					match opt.player_idx {
						0 => init.p1 = opt.kind.clone(),
						_ => init.p2 = opt.kind.clone(),
					}
					for (label, mut text) in &mut label_query {
						if label.0 == opt.player_idx {
							**text = format_player_label(&opt.kind, &ratings.0);
						}
					}
					for entity in &existing_dropdowns {
						commands.entity(entity).despawn();
					}
				} else if let Some(opt) = size_opt {
					init.size = opt.0;
					let n = u8::from(opt.0);
					for mut text in &mut size_label {
						**text = format!("{n}x{n}");
					}
					for entity in &existing_dropdowns {
						commands.entity(entity).despawn();
					}
				}
				*color = theme::BTN_PRESSED.into();
			}
			Interaction::Hovered =>
				if start.is_some() {
					*color = theme::BTN_START_HOVER.into();
				} else {
					*color = theme::BTN_HOVERED.into();
				},
			Interaction::None =>
				if start.is_some() {
					*color = theme::BTN_START.into();
				} else {
					*color = theme::BTN_NORMAL.into();
				},
		}
	}
}

fn dropdown_system(
	mouse: Res<ButtonInput<MouseButton>>,
	dropdowns: Query<Entity, With<DropdownPanel>>,
	interactions: Query<&Interaction, (With<Button>, Or<(With<PlayerButton>, With<SizeButton>, With<DropdownOption>, With<SizeDropdownOption>)>)>,
	mut commands: Commands,
) {
	if mouse.just_pressed(MouseButton::Left) && !dropdowns.is_empty() {
		let any_interaction = interactions.iter().any(|i| *i != Interaction::None);
		if !any_interaction {
			for entity in &dropdowns {
				commands.entity(entity).despawn();
			}
		}
	}
}

fn spawn_player_dropdown(commands: &mut Commands, player_idx: usize, ratings: &HashMap<Ustr, Rating>) {
	let mut kinds = PlayerKind::defaults();
	let default_ids: Vec<Ustr> = kinds.iter().map(|k| k.id()).collect();

	// Discover players persisted in the ratings DB that aren't already in the defaults
	for key in ratings.keys() {
		if default_ids.contains(key) {
			continue;
		}
		let s = key.as_str();
		match s.parse::<PlayerKind>() {
			Ok(kind) => kinds.push(kind),
			Err(_) =>
				if robot_master_arena::algos::validate_manual_name(s).is_ok() {
					kinds.push(PlayerKind::ManualPlayer(robot_master_arena::player::ManualPlayer { name: s.to_string() }));
				},
		}
	}

	kinds.sort_by(|a, b| {
		let ra = ratings.get(&a.id()).map(|e| e.rating).unwrap_or(f64::NEG_INFINITY);
		let rb = ratings.get(&b.id()).map(|e| e.rating).unwrap_or(f64::NEG_INFINITY);
		rb.partial_cmp(&ra).unwrap()
	});

	let dropdown_items: Vec<_> = kinds.iter().map(|kind| dropdown_item(player_idx, format_player_label(kind, ratings), kind.clone())).collect();

	commands
		.spawn((
			DropdownPanel,
			Node {
				position_type: PositionType::Absolute,
				left: Val::Percent(50.0),
				top: Val::Percent(if player_idx == 0 { 48.0 } else { 55.0 }),
				width: Val::Px(200.0),
				max_height: Val::Px(240.0),
				flex_direction: FlexDirection::Column,
				border_radius: BorderRadius::all(Val::Px(10.0)),
				overflow: Overflow::scroll_y(),
				..default()
			},
			BackgroundColor(theme::DROPDOWN_BG),
			GlobalZIndex(10),
		))
		.with_children(|parent| {
			for item in dropdown_items {
				parent.spawn(item);
			}
		});
}

fn spawn_size_dropdown(commands: &mut Commands) {
	let items: Vec<_> = BoardSize::iter()
		.map(|s| {
			let n = u8::from(s);
			size_dropdown_item(s, format!("{n}x{n}"))
		})
		.collect();

	commands
		.spawn((
			DropdownPanel,
			Node {
				position_type: PositionType::Absolute,
				left: Val::Percent(50.0),
				top: Val::Percent(62.0),
				width: Val::Px(200.0),
				flex_direction: FlexDirection::Column,
				border_radius: BorderRadius::all(Val::Px(10.0)),
				..default()
			},
			BackgroundColor(theme::DROPDOWN_BG),
			GlobalZIndex(10),
		))
		.with_children(|parent| {
			for item in items {
				parent.spawn(item);
			}
		});
}

fn dropdown_item(player_idx: usize, label: String, kind: PlayerKind) -> impl Bundle {
	(
		DropdownOption { player_idx, kind },
		Button,
		Node {
			width: Val::Percent(100.0),
			height: Val::Px(40.0),
			justify_content: JustifyContent::Center,
			align_items: AlignItems::Center,
			..default()
		},
		BackgroundColor(theme::BTN_NORMAL),
		children![(Text::new(label), TextFont { font_size: 20.0, ..default() }, TextColor(theme::TEXT_PRIMARY))],
	)
}

fn size_dropdown_item(size: BoardSize, label: String) -> impl Bundle {
	(
		SizeDropdownOption(size),
		Button,
		Node {
			width: Val::Percent(100.0),
			height: Val::Px(40.0),
			justify_content: JustifyContent::Center,
			align_items: AlignItems::Center,
			..default()
		},
		BackgroundColor(theme::BTN_NORMAL),
		children![(Text::new(label), TextFont { font_size: 20.0, ..default() }, TextColor(theme::TEXT_PRIMARY))],
	)
}

fn load_ratings() -> HashMap<Ustr, Rating> {
	#[cfg(not(target_arch = "wasm32"))]
	{
		use robot_master_arena::db::JsonRatingDb;

		let db = JsonRatingDb::default();
		use robot_master_arena::db::RatingDb;
		db.load_ratings()
	}
	#[cfg(target_arch = "wasm32")]
	HashMap::new()
}

fn keyboard_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>, dropdowns: Query<Entity, With<DropdownPanel>>, mut commands: Commands) {
	if keys.just_pressed(KeyCode::Escape) {
		for entity in &dropdowns {
			commands.entity(entity).despawn();
		}
	}
	if (keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && dropdowns.is_empty() {
		next_state.set(AppState::Playing);
	}
}

fn cleanup_menu(mut commands: Commands, query: Query<Entity, With<MenuScene>>, dropdowns: Query<Entity, With<DropdownPanel>>) {
	for entity in query.iter().chain(dropdowns.iter()) {
		commands.entity(entity).despawn();
	}
	commands.remove_resource::<Ratings>();
}
