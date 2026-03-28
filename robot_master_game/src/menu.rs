use bevy::prelude::*;

use crate::{AppState, InitialPlayers, PlayerKind};

const NORMAL: Color = Color::srgba(0.2, 0.2, 0.4, 0.8);
const HOVERED: Color = Color::srgba(0.3, 0.3, 0.6, 0.9);
const PRESSED: Color = Color::srgba(0.2, 0.8, 0.2, 0.9);
const START_NORMAL: Color = Color::srgba(0.2, 0.7, 0.2, 0.8);
const START_HOVERED: Color = Color::srgba(0.3, 0.9, 0.3, 1.0);
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(OnEnter(AppState::Menu), setup_menu)
			.add_systems(Update, (button_system, dropdown_system).run_if(in_state(AppState::Menu)))
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
struct DropdownOption {
	player_idx: usize,
	kind: PlayerKind,
}

#[derive(Component)]
struct DropdownPanel;

fn setup_menu(mut commands: Commands, init: Res<InitialPlayers>) {
	let p1_kind = PlayerKind::from_name(&init.p1);
	let p2_kind = PlayerKind::from_name(&init.p2);

	commands.spawn((
		MenuScene,
		Node {
			width: Val::Percent(100.0),
			height: Val::Percent(100.0),
			flex_direction: FlexDirection::Column,
			align_items: AlignItems::Center,
			justify_content: JustifyContent::Center,
			row_gap: Val::Px(20.0),
			..default()
		},
		BackgroundColor(Color::srgb(0.08, 0.08, 0.15)),
		children![
			// Title
			(Text::new("ROBOT MASTER"), TextFont { font_size: 64.0, ..default() }, TextColor(Color::srgb(1.0, 0.85, 0.0)),),
			// Player 1 button
			player_button(0, &p1_kind),
			// Player 2 button
			player_button(1, &p2_kind),
			// Start button
			(
				StartButton,
				Button,
				Node {
					width: Val::Px(200.0),
					height: Val::Px(60.0),
					justify_content: JustifyContent::Center,
					align_items: AlignItems::Center,
					margin: UiRect::top(Val::Px(20.0)),
					..default()
				},
				BackgroundColor(START_NORMAL),
				children![(Text::new("START"), TextFont { font_size: 36.0, ..default() }, TextColor(Color::WHITE),)],
			),
		],
	));
}

fn player_button(idx: usize, kind: &PlayerKind) -> impl Bundle {
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
			..default()
		},
		BackgroundColor(NORMAL),
		children![
			(Text::new(label), TextFont { font_size: 24.0, ..default() }, TextColor(Color::WHITE),),
			(
				PlayerLabel(idx),
				Text::new(kind.to_string()),
				TextFont { font_size: 22.0, ..default() },
				TextColor(Color::srgb(0.8, 0.8, 0.3)),
			),
		],
	)
}

fn button_system(
	mut interaction_query: Query<(&Interaction, &mut BackgroundColor, Option<&StartButton>, Option<&PlayerButton>, Option<&DropdownOption>), Changed<Interaction>>,
	mut next_state: ResMut<NextState<AppState>>,
	mut commands: Commands,
	existing_dropdowns: Query<Entity, With<DropdownPanel>>,
	mut init: ResMut<InitialPlayers>,
	mut label_query: Query<(&PlayerLabel, &mut Text)>,
) {
	for (interaction, mut color, start, player_btn, dropdown_opt) in &mut interaction_query {
		match *interaction {
			Interaction::Pressed => {
				if start.is_some() {
					next_state.set(AppState::Playing);
				} else if let Some(btn) = player_btn {
					// Toggle dropdown
					let has_dropdown = !existing_dropdowns.is_empty();
					for entity in &existing_dropdowns {
						commands.entity(entity).despawn();
					}
					if !has_dropdown {
						spawn_dropdown(&mut commands, btn.0);
					}
				} else if let Some(opt) = dropdown_opt {
					let name = match &opt.kind {
						PlayerKind::Manual { .. } => "manual",
						PlayerKind::Random => "random",
						PlayerKind::Greedy => "greedy",
						PlayerKind::Sadist => "sadist",
					};
					match opt.player_idx {
						0 => init.p1 = name.to_string(),
						_ => init.p2 = name.to_string(),
					}
					// Update label
					for (label, mut text) in &mut label_query {
						if label.0 == opt.player_idx {
							**text = opt.kind.to_string();
						}
					}
					// Close dropdown
					for entity in &existing_dropdowns {
						commands.entity(entity).despawn();
					}
				}
				*color = PRESSED.into();
			}
			Interaction::Hovered =>
				if start.is_some() {
					*color = START_HOVERED.into();
				} else {
					*color = HOVERED.into();
				},
			Interaction::None =>
				if start.is_some() {
					*color = START_NORMAL.into();
				} else {
					*color = NORMAL.into();
				},
		}
	}
}

fn dropdown_system(
	// Close dropdown when clicking outside
	mouse: Res<ButtonInput<MouseButton>>,
	dropdowns: Query<Entity, With<DropdownPanel>>,
	interactions: Query<&Interaction, (With<Button>, Or<(With<PlayerButton>, With<DropdownOption>)>)>,
	mut commands: Commands,
) {
	if mouse.just_pressed(MouseButton::Left) && !dropdowns.is_empty() {
		// If no button is being interacted with, close dropdown
		let any_interaction = interactions.iter().any(|i| *i != Interaction::None);
		if !any_interaction {
			for entity in &dropdowns {
				commands.entity(entity).despawn();
			}
		}
	}
}

fn spawn_dropdown(commands: &mut Commands, player_idx: usize) {
	commands.spawn((
		DropdownPanel,
		Node {
			position_type: PositionType::Absolute,
			left: Val::Percent(50.0),
			top: Val::Percent(if player_idx == 0 { 48.0 } else { 55.0 }),
			width: Val::Px(200.0),
			flex_direction: FlexDirection::Column,
			..default()
		},
		BackgroundColor(Color::srgba(0.15, 0.15, 0.25, 0.95)),
		GlobalZIndex(10),
		children![
			dropdown_item(player_idx, "Manual", PlayerKind::Manual { name: "Player".into() }),
			dropdown_item(player_idx, "Random", PlayerKind::Random),
			dropdown_item(player_idx, "Greedy", PlayerKind::Greedy),
			dropdown_item(player_idx, "Sadist", PlayerKind::Sadist),
		],
	));
}

fn dropdown_item(player_idx: usize, label: &str, kind: PlayerKind) -> impl Bundle {
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
		BackgroundColor(NORMAL),
		children![(Text::new(label), TextFont { font_size: 20.0, ..default() }, TextColor(Color::WHITE),)],
	)
}

fn cleanup_menu(mut commands: Commands, query: Query<Entity, With<MenuScene>>, dropdowns: Query<Entity, With<DropdownPanel>>) {
	for entity in query.iter().chain(dropdowns.iter()) {
		commands.entity(entity).despawn();
	}
}
