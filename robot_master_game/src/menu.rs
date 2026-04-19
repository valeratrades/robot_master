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
			.add_systems(Update, (button_system, search_system, keyboard_shortcuts, sync_start_button).run_if(in_state(AppState::Menu)))
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
struct SettingsButton;

/// Marker for the settings popup modal.
#[derive(Component)]
struct SettingsModal;

/// The size label shown inside the settings modal.
#[derive(Component)]
struct SizeLabel;

#[derive(Component)]
struct SizeDropdownOption(BoardSize);

/// One segment of the show/hide cards toggle. `true` = "Hide cards" option, `false` = "Show cards".
#[derive(Component)]
struct HideSegment(bool);

/// Button in the settings modal that opens the eval model search (player_idx == 2).
#[derive(Component)]
struct EvalModelButton;

/// Button in the settings modal that clears the selected eval model.
#[derive(Component)]
struct EvalModelClearButton;

/// Delete (nuke) button shown next to each item in the search results list.
#[derive(Component)]
struct SearchResultDeleteButton {
	kind: PlayerKind,
	player_idx: usize,
}

/// Button shown when the search returns no matches; creates a ManualPlayer from the query.
/// Shown only for player slots 0 and 1. Ratings are updated normally.
#[derive(Component)]
struct SearchNoMatchManualButton {
	query: String,
	player_idx: usize,
}

/// Button shown when the search returns no matches; parses the raw query as a PlayerKind spec and
/// starts the game in no-priors mode (slots 0/1) or sets the eval model (slot 2).
#[derive(Component)]
struct NoPriorsSelectButton {
	query: String,
	player_idx: usize,
}

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

fn setup_menu(mut commands: Commands, mut init: ResMut<InitialPlayers>, asset_server: Res<AssetServer>) {
	init.no_priors = false;
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
						panel.spawn((Text::new("ROBOT MASTER"), TextFont { font_size: 64.0, ..default() }, TextColor(theme::text::TITLE)));
						panel.spawn(player_button(0, format_player_label(&p1_kind, &ratings)));
						panel.spawn(player_button(1, format_player_label(&p2_kind, &ratings)));
						panel.spawn(settings_button());
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
							BackgroundColor(theme::btn::START),
							children![(Text::new("START"), TextFont { font_size: 36.0, ..default() }, TextColor(theme::text::PRIMARY))],
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
		BackgroundColor(theme::btn::NORMAL),
		children![
			(Text::new(label), TextFont { font_size: 24.0, ..default() }, TextColor(theme::text::PRIMARY)),
			(
				PlayerLabel(idx),
				Text::new(display),
				TextFont { font_size: 22.0, ..default() },
				TextColor(theme::Catppuccin::color(theme::Palette::Overlay2))
			),
		],
	)
}

fn settings_button() -> impl Bundle {
	(
		SettingsButton,
		Button,
		Node {
			width: Val::Px(400.0),
			height: Val::Px(50.0),
			justify_content: JustifyContent::Center,
			align_items: AlignItems::Center,
			border_radius: BorderRadius::all(Val::Px(10.0)),
			..default()
		},
		BackgroundColor(theme::btn::NORMAL),
		children![(Text::new("Settings"), TextFont { font_size: 24.0, ..default() }, TextColor(theme::text::PRIMARY))],
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
						constrain_sizes: None,
						constrain_hide: None,
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

fn spawn_search_modal(commands: &mut Commands, player_idx: usize, candidates: Vec<(String, PlayerKind)>, nerd_font: &Handle<Font>) {
	let initial_items: Vec<_> = candidates.clone();

	commands
		.spawn((
			SearchModal,
			SearchState {
				player_idx,
				candidates,
				highlighted: 0,
				last_query: String::default(),
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
			GlobalZIndex(theme::layer::MODAL_SEARCH),
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
							value: "Search player...".to_string(),
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
								spawn_result_item(list, label.clone(), kind.clone(), player_idx, i == 0, nerd_font);
							}
						});
				});
		});
}

fn spawn_result_item(parent: &mut ChildSpawnerCommands, label: String, kind: PlayerKind, player_idx: usize, highlighted: bool, nerd_font: &Handle<Font>) {
	let bg = if highlighted { theme::btn::HOVERED } else { theme::btn::NORMAL };
	parent
		.spawn(Node {
			width: Val::Percent(100.0),
			height: Val::Px(40.0),
			flex_direction: FlexDirection::Row,
			align_items: AlignItems::Center,
			column_gap: Val::Px(4.0),
			..default()
		})
		.with_children(|row| {
			row.spawn((
				SearchResultItem { kind: kind.clone(), player_idx },
				Button,
				Node {
					flex_grow: 1.0,
					height: Val::Percent(100.0),
					align_items: AlignItems::Center,
					padding: UiRect::horizontal(Val::Px(12.0)),
					border_radius: BorderRadius::all(Val::Px(6.0)),
					..default()
				},
				BackgroundColor(bg),
				children![(Text::new(label), TextFont { font_size: 20.0, ..default() }, TextColor(theme::text::PRIMARY))],
			));
			row.spawn((
				SearchResultDeleteButton { kind, player_idx },
				Button,
				Node {
					width: Val::Px(36.0),
					height: Val::Px(32.0),
					justify_content: JustifyContent::Center,
					align_items: AlignItems::Center,
					border_radius: BorderRadius::all(Val::Px(6.0)),
					..default()
				},
				BackgroundColor(theme::btn::NORMAL),
				children![(
					Text::new("\u{eab8}"),
					TextFont {
						font: nerd_font.clone(),
						font_size: 14.0,
						..default()
					},
					TextColor(theme::text::DANGER)
				)],
			));
		});
}

fn spawn_no_match_button<M: Component>(list: &mut ChildSpawnerCommands, marker: M, label: &str, shortcut: &str, shortcut_font: &Handle<Font>) {
	list.spawn((
		marker,
		Button,
		Node {
			width: Val::Percent(100.0),
			height: Val::Px(40.0),
			align_items: AlignItems::Center,
			padding: UiRect::horizontal(Val::Px(12.0)),
			border_radius: BorderRadius::all(Val::Px(6.0)),
			..default()
		},
		BackgroundColor(theme::btn::NORMAL),
	))
	.with_children(|btn| {
		btn.spawn((Text::new(label.to_string()), TextFont { font_size: 18.0, ..default() }, TextColor(theme::text::MUTED)));
		btn.spawn((
			Node {
				position_type: PositionType::Absolute,
				bottom: Val::Px(3.0),
				right: Val::Px(6.0),
				..default()
			},
			Text::new(shortcut.to_string()),
			TextFont {
				font: shortcut_font.clone(),
				font_size: 10.0,
				..default()
			},
			TextColor(theme::Catppuccin::color(theme::Palette::Surface2)),
		));
	});
}

// ── systems ───────────────────────────────────────────────────────────────────

fn button_system(
	mut interaction_query: Query<
		(
			&Interaction,
			&mut BackgroundColor,
			Option<&StartButton>,
			Option<&PlayerButton>,
			Option<&SettingsButton>,
			Option<&SizeDropdownOption>,
			Option<&HideSegment>,
			Option<&SearchResultItem>,
			Option<&EvalModelButton>,
			Option<&EvalModelClearButton>,
			Option<&SearchResultDeleteButton>,
			Option<&NoPriorsSelectButton>,
			Option<&SearchNoMatchManualButton>,
		),
		Changed<Interaction>,
	>,
	mut next_state: ResMut<NextState<AppState>>,
	mut commands: Commands,
	search_modals: Query<Entity, (With<SearchModal>, Without<SettingsModal>)>,
	settings_modals: Query<Entity, With<SettingsModal>>,
	mut init: ResMut<InitialPlayers>,
	mut label_query: Query<(&PlayerLabel, &mut Text), Without<SizeLabel>>,
	mut ratings: ResMut<Ratings>,
	nerd_font: Res<crate::NerdFont>,
) {
	for (
		interaction,
		mut color,
		start,
		player_btn,
		settings_btn,
		size_opt,
		hide_seg,
		result_item,
		eval_model_btn,
		eval_model_clear_btn,
		search_result_delete,
		no_priors_btn,
		no_match_manual_btn,
	) in &mut interaction_query
	{
		match *interaction {
			Interaction::Pressed => {
				if start.is_some() {
					// Guard: hide mode forbids two manual players.
					if init.hide && init.p1.is_manual() && init.p2.is_manual() {
						// do nothing - sync_start_button keeps it visually disabled
					} else {
						next_state.set(AppState::Playing);
					}
				} else if let Some(btn) = player_btn {
					for entity in &search_modals {
						commands.entity(entity).despawn();
					}
					let mut candidates = build_candidates(&ratings.0);
					// In hidden-hand mode, player 2 cannot be manual.
					if init.hide && btn.0 == 1 {
						candidates.retain(|(_, kind)| !kind.is_manual());
					}
					spawn_search_modal(&mut commands, btn.0, candidates, &nerd_font.0);
				} else if settings_btn.is_some() {
					let has_modal = !settings_modals.is_empty();
					for entity in &settings_modals {
						commands.entity(entity).despawn();
					}
					if !has_modal {
						spawn_settings_modal(&mut commands, init.size, init.hide, init.eval_model.as_ref(), &nerd_font.0);
					}
				} else if let Some(opt) = size_opt {
					init.size = opt.0;
					// Reopen the settings modal so the selected size is highlighted correctly.
					for entity in &settings_modals {
						commands.entity(entity).despawn();
					}
					spawn_settings_modal(&mut commands, init.size, init.hide, init.eval_model.as_ref(), &nerd_font.0);
				} else if let Some(seg) = hide_seg {
					if seg.0 != init.hide {
						init.hide = seg.0;
						// If hide just turned on and p2 is manual, reset p2 to random.
						if init.hide && init.p2.is_manual() {
							use robot_master_arena::algos::{InnerKind, RandomPlayer};
							init.p2 = PlayerKind {
								inner: InnerKind::RandomPlayer(RandomPlayer::default()),
								sims: None,
								constrain_sizes: None,
								constrain_hide: None,
							};
							for (label, mut text) in &mut label_query {
								if label.0 == 1 {
									**text = format_player_label(&init.p2.clone(), &ratings.0);
								}
							}
						}
						// Reopen the settings modal so segment highlight reflects new state.
						for entity in &settings_modals {
							commands.entity(entity).despawn();
						}
						spawn_settings_modal(&mut commands, init.size, init.hide, init.eval_model.as_ref(), &nerd_font.0);
					}
				} else if eval_model_btn.is_some() {
					for entity in &search_modals {
						commands.entity(entity).despawn();
					}
					let candidates = build_candidates(&ratings.0).into_iter().filter(|(_, k)| k.is_onnx()).collect();
					spawn_search_modal(&mut commands, 2, candidates, &nerd_font.0);
				} else if eval_model_clear_btn.is_some() {
					init.eval_model = None;
					for entity in &settings_modals {
						commands.entity(entity).despawn();
					}
					spawn_settings_modal(&mut commands, init.size, init.hide, None, &nerd_font.0);
				} else if let Some(item) = result_item {
					if item.player_idx == 2 {
						init.eval_model = Some(item.kind.clone());
						for entity in &search_modals {
							commands.entity(entity).despawn();
						}
						for entity in &settings_modals {
							commands.entity(entity).despawn();
						}
						spawn_settings_modal(&mut commands, init.size, init.hide, init.eval_model.as_ref(), &nerd_font.0);
					} else {
						match item.player_idx {
							0 => init.p1 = item.kind.clone(),
							_ => init.p2 = item.kind.clone(),
						}
						for (label, mut text) in &mut label_query {
							if label.0 == item.player_idx {
								**text = format_player_label(&item.kind, &ratings.0);
							}
						}
						for entity in &search_modals {
							commands.entity(entity).despawn();
						}
					}
				} else if let Some(del) = search_result_delete {
					#[cfg(not(target_arch = "wasm32"))]
					{
						use robot_master_arena::db::{JsonRatingDb, RatingDb};
						let db = JsonRatingDb::default();
						let mut r = db.load_ratings();
						r.remove(&del.kind.id());
						db.save_ratings(&r);
						ratings.0 = r;
					}
					let mut candidates = build_candidates(&ratings.0);
					if del.player_idx == 2 {
						candidates.retain(|(_, k)| k.is_onnx());
					} else if init.hide && del.player_idx == 1 {
						candidates.retain(|(_, kind)| !kind.is_manual());
					}
					for entity in &search_modals {
						commands.entity(entity).despawn();
					}
					spawn_search_modal(&mut commands, del.player_idx, candidates, &nerd_font.0);
				} else if let Some(btn) = no_priors_btn {
					if btn.player_idx == 2 {
						// Eval slot: only accept valid specs, no ManualPlayer fallback
						if let Ok(kind) = btn.query.parse::<PlayerKind>() {
							init.eval_model = Some(kind);
							for entity in &search_modals {
								commands.entity(entity).despawn();
							}
							for entity in &settings_modals {
								commands.entity(entity).despawn();
							}
							spawn_settings_modal(&mut commands, init.size, init.hide, init.eval_model.as_ref(), &nerd_font.0);
						}
					} else {
						use robot_master_arena::{algos::InnerKind, player::ManualPlayer as MP};
						let kind = btn.query.parse::<PlayerKind>().unwrap_or_else(|_| PlayerKind {
							inner: InnerKind::ManualPlayer(MP { name: btn.query.clone() }),
							sims: None,
							constrain_sizes: None,
							constrain_hide: None,
						});
						match btn.player_idx {
							0 => init.p1 = kind.clone(),
							_ => init.p2 = kind.clone(),
						}
						for (label, mut text) in &mut label_query {
							if label.0 == btn.player_idx {
								**text = format_player_label(&kind, &ratings.0);
							}
						}
						init.no_priors = true;
						for entity in &search_modals {
							commands.entity(entity).despawn();
						}
					}
				} else if let Some(btn) = no_match_manual_btn {
					use robot_master_arena::{algos::InnerKind, player::ManualPlayer as MP};
					let kind = PlayerKind {
						inner: InnerKind::ManualPlayer(MP { name: btn.query.clone() }),
						sims: None,
						constrain_sizes: None,
						constrain_hide: None,
					};
					match btn.player_idx {
						0 => init.p1 = kind.clone(),
						_ => init.p2 = kind.clone(),
					}
					for (label, mut text) in &mut label_query {
						if label.0 == btn.player_idx {
							**text = format_player_label(&kind, &ratings.0);
						}
					}
					for entity in &search_modals {
						commands.entity(entity).despawn();
					}
				}
				*color = theme::btn::PRESSED.into();
			}
			Interaction::Hovered =>
				if result_item.is_none() {
					*color = if start.is_some() {
						theme::btn::START.lighter(0.1).into()
					} else {
						theme::btn::HOVERED.into()
					};
				},
			Interaction::None =>
				if start.is_some() {
					let blocked = init.hide && init.p1.is_manual() && init.p2.is_manual();
					*color = if blocked { theme::btn::NORMAL.into() } else { theme::btn::START.into() };
				} else if let Some(opt) = size_opt {
					*color = if opt.0 == init.size { theme::btn::HOVERED.into() } else { theme::btn::NORMAL.into() };
				} else if let Some(seg) = hide_seg {
					*color = if seg.0 == init.hide { theme::btn::HOVERED.into() } else { theme::btn::NORMAL.into() };
				} else if result_item.is_none() {
					*color = theme::btn::NORMAL.into();
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
	settings_modals: Query<Entity, With<SettingsModal>>,
	nerd_font: Res<crate::NerdFont>,
	noto_symbols: Res<crate::NotoSymbolsFont>,
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

	let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
	if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
		use robot_master_arena::{algos::InnerKind, player::ManualPlayer as MP};
		let hide = init.hide;
		let player_idx = state.player_idx;
		let make_manual = || PlayerKind {
			inner: InnerKind::ManualPlayer(MP { name: query_str.clone() }),
			sims: None,
			constrain_sizes: None,
			constrain_hide: None,
		};
		// (kind, no_priors)
		let resolution: Option<(PlayerKind, bool)> = if ctrl && !query_str.is_empty() && !(hide && player_idx == 1) {
			// Ctrl+Enter always constructs from the typed query, ignoring any matches.
			if player_idx == 2 {
				query_str.parse::<PlayerKind>().ok().map(|k| (k, false))
			} else {
				Some((query_str.parse::<PlayerKind>().unwrap_or_else(|_| make_manual()), true))
			}
		} else if let Some((_, k)) = filtered.get(state.highlighted) {
			Some((k.clone(), false))
		} else if query_str.is_empty() || (hide && player_idx == 1) {
			None
		} else if player_idx == 2 {
			// Eval slot: Enter → parse as spec
			query_str.parse::<PlayerKind>().ok().map(|k| (k, false))
		} else {
			// Plain Enter → manual player
			Some((make_manual(), false))
		};
		if let Some((kind, no_priors)) = resolution {
			if player_idx == 2 {
				init.eval_model = Some(kind);
				commands.entity(modal_entity).despawn();
				for entity in &settings_modals {
					commands.entity(entity).despawn();
				}
				spawn_settings_modal(&mut commands, init.size, init.hide, init.eval_model.as_ref(), &nerd_font.0);
			} else {
				match player_idx {
					0 => init.p1 = kind.clone(),
					_ => init.p2 = kind.clone(),
				}
				for (label, mut text) in &mut label_query {
					if label.0 == player_idx {
						**text = format_player_label(&kind, &ratings.0);
					}
				}
				if no_priors {
					init.no_priors = true;
				}
				commands.entity(modal_entity).despawn();
			}
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
			spawn_result_item(list, label.clone(), kind.clone(), player_idx, i == highlighted, &nerd_font.0);
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
				children![(Text::new("no matches"), TextFont { font_size: 18.0, ..default() }, TextColor(theme::text::MUTED))],
			));
			if !query_str.is_empty() {
				// Manual button only for player slots 0/1 (not eval)
				if player_idx != 2 {
					spawn_no_match_button(
						list,
						SearchNoMatchManualButton {
							query: query_str.clone(),
							player_idx,
						},
						"New manual player",
						"\u{23ce}",
						&noto_symbols.0,
					);
				}
				// Temp algo button for all slots
				let (algo_label, algo_shortcut) = if player_idx == 2 {
					("Use as eval model", "\u{23ce}")
				} else {
					("New temp algo", "Ctrl+\u{23ce}")
				};
				spawn_no_match_button(
					list,
					NoPriorsSelectButton {
						query: query_str.clone(),
						player_idx,
					},
					algo_label,
					algo_shortcut,
					&noto_symbols.0,
				);
			}
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
	scored.sort_by_key(|b| std::cmp::Reverse(b.0));
	scored.into_iter().map(|(_, label, kind)| (label, kind)).collect()
}

// ── settings modal ────────────────────────────────────────────────────────────

fn spawn_settings_modal(commands: &mut Commands, current_size: BoardSize, hide: bool, eval_model: Option<&PlayerKind>, nerd_font: &Handle<Font>) {
	commands
		.spawn((
			SettingsModal,
			Node {
				position_type: PositionType::Absolute,
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(Color::oklcha(0.0, 0.0, 0.0, 0.6)),
			GlobalZIndex(theme::layer::MODAL),
		))
		.with_children(|overlay| {
			overlay
				.spawn((
					Node {
						flex_direction: FlexDirection::Column,
						width: Val::Px(320.0),
						padding: UiRect::all(Val::Px(20.0)),
						row_gap: Val::Px(12.0),
						border_radius: BorderRadius::all(Val::Px(16.0)),
						..default()
					},
					BackgroundColor(Color::oklcha(0.18, 0.03, 260.0, 0.98)),
				))
				.with_children(|modal| {
					// Header
					modal.spawn((Text::new("Settings"), TextFont { font_size: 26.0, ..default() }, TextColor(theme::text::TITLE)));

					// Board size section label
					modal.spawn((
						Text::new("Board Size"),
						TextFont { font_size: 18.0, ..default() },
						TextColor(theme::Catppuccin::color(theme::Palette::Overlay2)),
					));

					// Size options row
					modal
						.spawn(Node {
							flex_direction: FlexDirection::Row,
							column_gap: Val::Px(8.0),
							..default()
						})
						.with_children(|row| {
							for size in BoardSize::iter() {
								let n = u8::from(size);
								let is_current = size == current_size;
								row.spawn((
									SizeDropdownOption(size),
									Button,
									Node {
										width: Val::Px(60.0),
										height: Val::Px(44.0),
										justify_content: JustifyContent::Center,
										align_items: AlignItems::Center,
										border_radius: BorderRadius::all(Val::Px(8.0)),
										..default()
									},
									BackgroundColor(if is_current { theme::btn::HOVERED } else { theme::btn::NORMAL }),
									children![(
										SizeLabel,
										Text::new(format!("{n}x{n}")),
										TextFont { font_size: 18.0, ..default() },
										TextColor(theme::text::PRIMARY)
									)],
								));
							}
						});

					// Opponent cards segmented toggle
					modal.spawn((
						Text::new("Opponent Cards"),
						TextFont { font_size: 18.0, ..default() },
						TextColor(theme::Catppuccin::color(theme::Palette::Overlay2)),
					));
					modal
						.spawn(Node {
							flex_direction: FlexDirection::Row,
							width: Val::Percent(100.0),
							..default()
						})
						.with_children(|row| {
							for (value, label) in [(false, "Show"), (true, "Hide")] {
								let active = value == hide;
								row.spawn((
									HideSegment(value),
									Button,
									Node {
										flex_grow: 1.0,
										height: Val::Px(44.0),
										justify_content: JustifyContent::Center,
										align_items: AlignItems::Center,
										border_radius: if value { BorderRadius::right(Val::Px(8.0)) } else { BorderRadius::left(Val::Px(8.0)) },
										..default()
									},
									BackgroundColor(if active { theme::btn::HOVERED } else { theme::btn::NORMAL }),
									children![(
										Text::new(label),
										TextFont { font_size: 18.0, ..default() },
										TextColor(if active { theme::text::PRIMARY } else { theme::text::MUTED })
									)],
								));
							}
						});

					// Eval model selector
					modal.spawn((
						Text::new("Eval Model"),
						TextFont { font_size: 18.0, ..default() },
						TextColor(theme::Catppuccin::color(theme::Palette::Overlay2)),
					));
					modal
						.spawn(Node {
							flex_direction: FlexDirection::Row,
							width: Val::Percent(100.0),
							column_gap: Val::Px(6.0),
							..default()
						})
						.with_children(|row| {
							let label = eval_model.map(|k: &PlayerKind| k.to_string()).unwrap_or_else(|| "None".to_string());
							row.spawn((
								EvalModelButton,
								Button,
								Node {
									flex_grow: 1.0,
									height: Val::Px(44.0),
									justify_content: JustifyContent::Center,
									align_items: AlignItems::Center,
									border_radius: BorderRadius::all(Val::Px(8.0)),
									..default()
								},
								BackgroundColor(theme::btn::NORMAL),
								children![(Text::new(label), TextFont { font_size: 16.0, ..default() }, TextColor(theme::text::PRIMARY))],
							));
							if eval_model.is_some() {
								row.spawn((
									EvalModelClearButton,
									Button,
									Node {
										width: Val::Px(36.0),
										height: Val::Px(44.0),
										justify_content: JustifyContent::Center,
										align_items: AlignItems::Center,
										border_radius: BorderRadius::all(Val::Px(8.0)),
										..default()
									},
									BackgroundColor(theme::btn::NORMAL),
									children![(
										Text::new("\u{eab8}"),
										TextFont {
											font: nerd_font.clone(),
											font_size: 16.0,
											..default()
										},
										TextColor(theme::text::DANGER)
									)],
								));
							}
						});
				});
		});
}

fn keyboard_shortcuts(
	keys: Res<ButtonInput<KeyCode>>,
	mut next_state: ResMut<NextState<AppState>>,
	search_modals: Query<Entity, (With<SearchModal>, Without<SettingsModal>)>,
	settings_modals: Query<Entity, With<SettingsModal>>,
	mut commands: Commands,
	init: Res<InitialPlayers>,
) {
	if keys.just_pressed(KeyCode::Escape) {
		for entity in search_modals.iter().chain(settings_modals.iter()) {
			commands.entity(entity).despawn();
		}
		return;
	}
	let no_modals = search_modals.is_empty() && settings_modals.is_empty();
	let can_start = !(init.hide && init.p1.is_manual() && init.p2.is_manual());
	if (keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && no_modals && can_start {
		next_state.set(AppState::Playing);
	}
}

/// Keep the Start button colour reflecting whether a valid config is selected.
fn sync_start_button(init: Res<InitialPlayers>, mut query: Query<&mut BackgroundColor, With<StartButton>>) {
	if !init.is_changed() {
		return;
	}
	let blocked = init.hide && init.p1.is_manual() && init.p2.is_manual();
	for mut color in &mut query {
		*color = if blocked { BackgroundColor(theme::btn::NORMAL) } else { BackgroundColor(theme::btn::START) };
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
	HashMap::default()
}

fn cleanup_menu(
	mut commands: Commands,
	scenes: Query<Entity, With<MenuScene>>,
	search_modals: Query<Entity, (With<SearchModal>, Without<SettingsModal>)>,
	settings_modals: Query<Entity, With<SettingsModal>>,
) {
	for entity in scenes.iter().chain(search_modals.iter()).chain(settings_modals.iter()) {
		commands.entity(entity).despawn();
	}
	commands.remove_resource::<Ratings>();
}
