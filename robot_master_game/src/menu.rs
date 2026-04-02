use std::collections::HashMap;

use bevy::prelude::*;
use bevy_simple_text_input::{TextInput, TextInputInactive, TextInputPlaceholder, TextInputPlugin, TextInputSettings, TextInputValue};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
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
		app.add_plugins(TextInputPlugin)
			.add_systems(OnEnter(AppState::Menu), setup_menu)
			.add_systems(Update, (button_system, search_system, keyboard_shortcuts).run_if(in_state(AppState::Menu)))
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
struct SizeDropdownOption(BoardSize);

/// The full-screen search modal overlay.
#[derive(Component)]
struct SearchModal;

/// Marker for the scrollable results list container inside the modal.
#[derive(Component)]
struct SearchResultsList;

/// One result row in the filtered list.
#[derive(Component)]
struct SearchResultItem {
	kind: PlayerKind,
	player_idx: usize,
}

/// State held on the modal entity so systems can read/write it.
#[derive(Component)]
struct SearchState {
	player_idx: usize,
	/// All candidates (label, kind), pre-sorted by Elo.
	candidates: Vec<(String, PlayerKind)>,
	/// Currently highlighted row index in the *filtered* list.
	highlighted: usize,
	/// The query that was used to build the current visible list.
	last_query: String,
}

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

// ── search modal ──────────────────────────────────────────────────────────────

fn build_candidates(ratings: &HashMap<Ustr, Rating>) -> Vec<(String, PlayerKind)> {
	let mut kinds = PlayerKind::defaults();
	let default_ids: Vec<Ustr> = kinds.iter().map(|k| k.id()).collect();

	for key in ratings.keys() {
		if default_ids.contains(key) {
			continue;
		}
		let s = key.as_str();
		match s.parse::<PlayerKind>() {
			Ok(kind) => kinds.push(kind),
			Err(_) =>
				if robot_master_arena::algos::validate_manual_name(s).is_ok() {
					kinds.push(PlayerKind {
						inner: robot_master_arena::algos::InnerKind::ManualPlayer(robot_master_arena::player::ManualPlayer { name: s.to_string() }),
						sims: None,
					});
				},
		}
	}

	kinds.sort_by(|a, b| {
		let ra = ratings.get(&a.id()).map(|e| e.rating).unwrap_or(f64::NEG_INFINITY);
		let rb = ratings.get(&b.id()).map(|e| e.rating).unwrap_or(f64::NEG_INFINITY);
		rb.partial_cmp(&ra).unwrap()
	});

	kinds.iter().map(|k| (format_player_label(k, ratings), k.clone())).collect()
}

fn spawn_search_modal(commands: &mut Commands, player_idx: usize, candidates: Vec<(String, PlayerKind)>) {
	let initial_items: Vec<_> = candidates.clone();

	commands
		.spawn((
			SearchModal,
			SearchState {
				player_idx,
				candidates,
				highlighted: 0,
				last_query: String::new(),
			},
			Node {
				position_type: PositionType::Absolute,
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(Color::oklcha(0.0, 0.0, 0.0, 0.6)),
			GlobalZIndex(20),
		))
		.with_children(|overlay| {
			overlay
				.spawn((
					Node {
						flex_direction: FlexDirection::Column,
						width: Val::Px(480.0),
						max_height: Val::Px(520.0),
						padding: UiRect::all(Val::Px(16.0)),
						row_gap: Val::Px(10.0),
						border_radius: BorderRadius::all(Val::Px(16.0)),
						..default()
					},
					BackgroundColor(Color::oklcha(0.18, 0.03, 260.0, 0.98)),
				))
				.with_children(|modal| {
					// ── search input ──────────────────────────────────────
					modal.spawn((
						TextInput,
						TextInputSettings {
							retain_on_submit: true,
							..default()
						},
						TextInputInactive(false),
						TextInputPlaceholder {
							value: "Search player…".to_string(),
							..default()
						},
						Node {
							width: Val::Percent(100.0),
							height: Val::Px(44.0),
							padding: UiRect::horizontal(Val::Px(12.0)),
							border: UiRect::all(Val::Px(1.0)),
							border_radius: BorderRadius::all(Val::Px(8.0)),
							..default()
						},
						BorderColor::all(Color::oklcha(0.50, 0.08, 260.0, 0.8)),
						BackgroundColor(Color::oklcha(0.25, 0.03, 260.0, 1.0)),
					));

					// ── results list ──────────────────────────────────────
					modal
						.spawn((
							SearchResultsList,
							Node {
								flex_direction: FlexDirection::Column,
								width: Val::Percent(100.0),
								overflow: Overflow::scroll_y(),
								..default()
							},
						))
						.with_children(|list| {
							for (i, (label, kind)) in initial_items.iter().enumerate() {
								list.spawn(result_item_bundle(label.clone(), kind.clone(), player_idx, i == 0));
							}
						});
				});
		});
}

fn result_item_bundle(label: String, kind: PlayerKind, player_idx: usize, highlighted: bool) -> impl Bundle {
	let bg = if highlighted { theme::BTN_HOVERED } else { theme::BTN_NORMAL };
	(
		SearchResultItem { kind, player_idx },
		Button,
		Node {
			width: Val::Percent(100.0),
			height: Val::Px(40.0),
			align_items: AlignItems::Center,
			padding: UiRect::horizontal(Val::Px(12.0)),
			border_radius: BorderRadius::all(Val::Px(6.0)),
			..default()
		},
		BackgroundColor(bg),
		children![(Text::new(label), TextFont { font_size: 20.0, ..default() }, TextColor(theme::TEXT_PRIMARY))],
	)
}

// ── systems ───────────────────────────────────────────────────────────────────

fn button_system(
	mut interaction_query: Query<
		(
			&Interaction,
			&mut BackgroundColor,
			Option<&StartButton>,
			Option<&PlayerButton>,
			Option<&SizeButton>,
			Option<&SizeDropdownOption>,
			Option<&SearchResultItem>,
		),
		Changed<Interaction>,
	>,
	mut next_state: ResMut<NextState<AppState>>,
	mut commands: Commands,
	modals: Query<Entity, With<SearchModal>>,
	mut init: ResMut<InitialPlayers>,
	mut label_query: Query<(&PlayerLabel, &mut Text), Without<SizeLabel>>,
	mut size_label: Query<&mut Text, With<SizeLabel>>,
	ratings: Res<Ratings>,
) {
	for (interaction, mut color, start, player_btn, size_btn, size_opt, result_item) in &mut interaction_query {
		match *interaction {
			Interaction::Pressed => {
				if start.is_some() {
					next_state.set(AppState::Playing);
				} else if let Some(btn) = player_btn {
					for entity in &modals {
						commands.entity(entity).despawn();
					}
					let mut candidates = build_candidates(&ratings.0);
					// In hidden-hand mode, player 2 cannot be manual.
					if init.hide && btn.0 == 1 {
						candidates.retain(|(_, kind)| !kind.is_manual());
					}
					spawn_search_modal(&mut commands, btn.0, candidates);
				} else if size_btn.is_some() {
					let has_modal = !modals.is_empty();
					for entity in &modals {
						commands.entity(entity).despawn();
					}
					if !has_modal {
						spawn_size_modal(&mut commands);
					}
				} else if let Some(opt) = size_opt {
					init.size = opt.0;
					let n = u8::from(opt.0);
					for mut text in &mut size_label {
						**text = format!("{n}x{n}");
					}
					for entity in &modals {
						commands.entity(entity).despawn();
					}
				} else if let Some(item) = result_item {
					match item.player_idx {
						0 => init.p1 = item.kind.clone(),
						_ => init.p2 = item.kind.clone(),
					}
					for (label, mut text) in &mut label_query {
						if label.0 == item.player_idx {
							**text = format_player_label(&item.kind, &ratings.0);
						}
					}
					for entity in &modals {
						commands.entity(entity).despawn();
					}
				}
				*color = theme::BTN_PRESSED.into();
			}
			Interaction::Hovered =>
				if result_item.is_none() {
					*color = if start.is_some() { theme::BTN_START_HOVER.into() } else { theme::BTN_HOVERED.into() };
				},
			Interaction::None =>
				if start.is_some() {
					*color = theme::BTN_START.into();
				} else if result_item.is_none() {
					*color = theme::BTN_NORMAL.into();
				},
		}
	}
}

/// Runs every frame; re-filters and rebuilds the results list when the query or
/// highlighted index changes.
fn search_system(
	mut modal_query: Query<(Entity, &mut SearchState), With<SearchModal>>,
	text_input_query: Query<&TextInputValue>,
	list_query: Query<Entity, With<SearchResultsList>>,
	keys: Res<ButtonInput<KeyCode>>,
	mut commands: Commands,
	mut init: ResMut<InitialPlayers>,
	mut label_query: Query<(&PlayerLabel, &mut Text), Without<SizeLabel>>,
	ratings: Res<Ratings>,
) {
	let Ok((modal_entity, mut state)) = modal_query.single_mut() else {
		return;
	};

	let query_str = text_input_query.iter().next().map(|v| v.0.clone()).unwrap_or_default();

	let filtered = filter_candidates(&state.candidates, &query_str);
	let item_count = filtered.len().max(1);

	let mut highlight_changed = false;
	if keys.just_pressed(KeyCode::ArrowDown) {
		state.highlighted = (state.highlighted + 1).min(item_count - 1);
		highlight_changed = true;
	}
	if keys.just_pressed(KeyCode::ArrowUp) {
		state.highlighted = state.highlighted.saturating_sub(1);
		highlight_changed = true;
	}

	if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
		let hide = init.hide;
		let player_idx = state.player_idx;
		let kind: Option<PlayerKind> = if let Some((_, k)) = filtered.get(state.highlighted) {
			Some(k.clone())
		} else if !query_str.is_empty() && !(hide && player_idx == 1) {
			// No matches — treat raw input as a manual player name.
			// Not allowed for player 2 in hidden-hand mode.
			Some(PlayerKind {
				inner: robot_master_arena::algos::InnerKind::ManualPlayer(robot_master_arena::player::ManualPlayer { name: query_str.clone() }),
				sims: None,
			})
		} else {
			None
		};
		if let Some(kind) = kind {
			match player_idx {
				0 => init.p1 = kind.clone(),
				_ => init.p2 = kind.clone(),
			}
			for (label, mut text) in &mut label_query {
				if label.0 == player_idx {
					**text = format_player_label(&kind, &ratings.0);
				}
			}
			commands.entity(modal_entity).despawn();
		}
		return;
	}

	if query_str == state.last_query && !highlight_changed {
		return;
	}
	if query_str != state.last_query {
		state.highlighted = 0;
	}
	state.last_query = query_str.clone();
	let highlighted = state.highlighted;

	let player_idx = state.player_idx;
	let Ok(list_entity) = list_query.single() else {
		return;
	};

	commands.entity(list_entity).despawn_related::<Children>();
	commands.entity(list_entity).with_children(|list| {
		for (i, (label, kind)) in filtered.iter().enumerate() {
			list.spawn(result_item_bundle(label.clone(), kind.clone(), player_idx, i == highlighted));
		}
		if filtered.is_empty() {
			list.spawn((
				Node {
					width: Val::Percent(100.0),
					height: Val::Px(40.0),
					align_items: AlignItems::Center,
					padding: UiRect::horizontal(Val::Px(12.0)),
					..default()
				},
				children![(Text::new("no matches"), TextFont { font_size: 18.0, ..default() }, TextColor(theme::TEXT_MUTED))],
			));
		}
	});
}

fn filter_candidates(candidates: &[(String, PlayerKind)], query: &str) -> Vec<(String, PlayerKind)> {
	if query.is_empty() {
		return candidates.to_vec();
	}
	let matcher = SkimMatcherV2::default();
	let mut scored: Vec<_> = candidates
		.iter()
		.filter_map(|(label, kind)| matcher.fuzzy_match(label, query).map(|score| (score, label.clone(), kind.clone())))
		.collect();
	scored.sort_by(|a, b| b.0.cmp(&a.0));
	scored.into_iter().map(|(_, label, kind)| (label, kind)).collect()
}

// ── size modal ────────────────────────────────────────────────────────────────

fn spawn_size_modal(commands: &mut Commands) {
	commands
		.spawn((
			SearchModal,
			Node {
				position_type: PositionType::Absolute,
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(Color::oklcha(0.0, 0.0, 0.0, 0.6)),
			GlobalZIndex(20),
		))
		.with_children(|overlay| {
			overlay
				.spawn((
					Node {
						flex_direction: FlexDirection::Column,
						width: Val::Px(240.0),
						padding: UiRect::all(Val::Px(16.0)),
						row_gap: Val::Px(8.0),
						border_radius: BorderRadius::all(Val::Px(16.0)),
						..default()
					},
					BackgroundColor(Color::oklcha(0.18, 0.03, 260.0, 0.98)),
				))
				.with_children(|modal| {
					for size in BoardSize::iter() {
						let n = u8::from(size);
						modal.spawn((
							SizeDropdownOption(size),
							Button,
							Node {
								width: Val::Percent(100.0),
								height: Val::Px(44.0),
								justify_content: JustifyContent::Center,
								align_items: AlignItems::Center,
								border_radius: BorderRadius::all(Val::Px(8.0)),
								..default()
							},
							BackgroundColor(theme::BTN_NORMAL),
							children![(Text::new(format!("{n}x{n}")), TextFont { font_size: 22.0, ..default() }, TextColor(theme::TEXT_PRIMARY))],
						));
					}
				});
		});
}

fn keyboard_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>, modals: Query<Entity, With<SearchModal>>, mut commands: Commands) {
	if keys.just_pressed(KeyCode::Escape) {
		for entity in &modals {
			commands.entity(entity).despawn();
		}
		return;
	}
	if (keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && modals.is_empty() {
		next_state.set(AppState::Playing);
	}
}

fn load_ratings() -> HashMap<Ustr, Rating> {
	#[cfg(not(target_arch = "wasm32"))]
	{
		use robot_master_arena::db::{JsonRatingDb, RatingDb};
		let db = JsonRatingDb::default();
		db.load_ratings()
	}
	#[cfg(target_arch = "wasm32")]
	HashMap::new()
}

fn cleanup_menu(mut commands: Commands, query: Query<Entity, With<MenuScene>>, modals: Query<Entity, With<SearchModal>>) {
	for entity in query.iter().chain(modals.iter()) {
		commands.entity(entity).despawn();
	}
	commands.remove_resource::<Ratings>();
}
