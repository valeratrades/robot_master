use std::ops::ControlFlow;

use bevy::{ecs::message::MessageReader, prelude::*};
use robot_master_arena::{
	BoardSize,
	algos::PlayerKind,
	match_::{DynMatch, Match},
};
use robot_master_core::{
	board::{EMPTY, Pos},
	cards::CardValue,
	game::{GameConfig, GameState, Move, Player, PlayerDisplay, PlayerSigned},
};
use robot_master_train::player_kind::kind_into_bot;
use v_utils::bevy::{ModalActionFired, ModalState, PressedChars};

use crate::{AppState, InitialPlayers, Textures, theme};

const WARNING_DURATION: f32 = 2.0;
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<ModalState<GameAction>>()
			.init_resource::<Warning>()
			.add_message::<ModalActionFired<GameAction>>()
			.add_systems(OnEnter(AppState::Playing), setup_gameplay)
			.add_systems(
				Update,
				((
					ai_turn, hand_click, keyboard_card_select, rebuild_modal_tree, process_modal_input, handle_modal_action, board_click, sync_visuals, sync_command_line,
					reject_flash_system, check_terminal, handle_escape,
				)
					.chain(),)
					.run_if(in_state(AppState::Playing)),
			)
			.add_systems(OnExit(AppState::Playing), cleanup_gameplay);
	}
}

// -- Game actions triggered by modal key sequences --

#[derive(Clone, Debug)]
enum GameAction {
	Exit,
	PlaceCard(Pos),
}

// -- Components & Resources --

#[derive(Component)]
struct GameScene;

#[derive(Resource)]
pub(crate) struct Game(pub(crate) Box<dyn DynMatch + Send + Sync>);

#[derive(Default, Resource)]
struct SelectedCard(Option<CardValue>);

#[derive(Resource)]
pub(crate) struct PlayerSlots(pub(crate) [PlayerKind; 2]);

#[derive(Component)]
struct BoardCell {
	row: u8,
	col: u8,
}

#[derive(Component)]
struct HandCard {
	player: Player,
	value: CardValue,
}

#[derive(Component)]
struct HandCountLabel {
	player: Player,
	value: CardValue,
}

#[derive(Component)]
struct TurnIndicator;

/// Persistent text element at the bottom showing ongoing key sequence or warnings.
#[derive(Component)]
struct CommandLine;

/// Brief warning message shown on the command line after invalid input.
#[derive(Default, Resource)]
struct Warning {
	text: String,
	timer: f32,
}

/// Helper: create a `Box<dyn DynMatch>` for the given board size.
fn make_match(size: BoardSize, hide: bool, p1: PlayerKind, p2: PlayerKind, models_dir: &std::path::Path) -> Box<dyn DynMatch + Send + Sync> {
	let mut rng: rand::rngs::SmallRng = rand::make_rng();
	let config = GameConfig { size: size.into(), hide };
	let p1_id = p1.id();
	let p2_id = p2.id();

	macro_rules! go {
		($N:literal) => {{
			let game = GameState::<$N>::new(config, &mut rng, [PlayerSigned::new(Player::A), PlayerSigned::new(Player::B)]);
			let p1 = kind_into_bot::<$N>(&p1, models_dir).unwrap_or_else(|e| panic!("{e}"));
			let p2 = kind_into_bot::<$N>(&p2, models_dir).unwrap_or_else(|e| panic!("{e}"));
			Box::new(Match::new(game, p1, p2, p1_id, p2_id))
		}};
	}

	match size {
		BoardSize::Five => go!(5),
		BoardSize::Seven => go!(7),
		BoardSize::Nine => go!(9),
		BoardSize::Eleven => go!(11),
	}
}

/// Snapshot of initial board state for spawning UI entities before inserting the Game resource.
struct InitialBoard {
	n: usize,
	cells: Vec<u8>,
	hands: [Vec<u8>; 2],
}

impl InitialBoard {
	fn get(&self, row: u8, col: u8) -> u8 {
		self.cells[row as usize * self.n + col as usize]
	}
}

fn setup_gameplay(mut commands: Commands, init: Res<InitialPlayers>, tex: Res<Textures>) {
	let size = init.size;
	let n = u8::from(size) as usize;
	let hide = init.hide;

	let p1_kind = init.p1.clone();
	let p2_kind = init.p2.clone();
	let models_dir = init.models_dir.clone();

	let m = make_match(size, hide, p1_kind.clone(), p2_kind.clone(), &models_dir);

	// Snapshot initial state before handing ownership to the resource.
	// In hidden mode Player B's hand is never shown, so use an empty placeholder for B.
	let hands = if hide { [m.p1_hand(), vec![0u8; 6]] } else { m.hands() };
	let mut cells = Vec::with_capacity(n * n);
	for r in 0..n as u8 {
		for c in 0..n as u8 {
			cells.push(m.get(Pos { row: r, col: c }));
		}
	}
	let snap = InitialBoard { n, cells, hands };

	commands.insert_resource(Game(m));
	commands.insert_resource(SelectedCard::default());
	commands.insert_resource(PlayerSlots([p1_kind, p2_kind]));

	let cell_px = 420.0 / n as f32;
	let img_px = cell_px - 10.0;

	commands
		.spawn((
			GameScene,
			Node {
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				flex_direction: FlexDirection::Column,
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(theme::BG_DARK),
		))
		.with_children(|root| {
			root.spawn((
				TurnIndicator,
				Text::new(""),
				TextFont { font_size: 24.0, ..default() },
				TextColor(theme::TEXT_PRIMARY),
				Node {
					margin: UiRect::bottom(Val::Px(10.0)),
					..default()
				},
			));

			root.spawn(Node {
				flex_direction: FlexDirection::Row,
				align_items: AlignItems::Center,
				column_gap: Val::Px(20.0),
				..default()
			})
			.with_children(|row| {
				spawn_hand(row, &snap.hands, Player::A, false, &tex);

				row.spawn(Node {
					flex_direction: FlexDirection::Column,
					..default()
				})
				.with_children(|board| {
					for r in 0..n as u8 {
						board
							.spawn(Node {
								flex_direction: FlexDirection::Row,
								..default()
							})
							.with_children(|board_row| {
								for c in 0..n as u8 {
									let val = snap.get(r, c);
									board_row
										.spawn((
											BoardCell { row: r, col: c },
											Button,
											Node {
												width: Val::Px(cell_px),
												height: Val::Px(cell_px),
												margin: UiRect::all(Val::Px(2.0)),
												justify_content: JustifyContent::Center,
												align_items: AlignItems::Center,
												..default()
											},
											BackgroundColor(if val != EMPTY { theme::CELL_OCCUPIED } else { theme::CELL_EMPTY }),
										))
										.with_children(|cell| {
											cell.spawn((
												ImageNode::new(if val != EMPTY { tex.card_face(CardValue(val)) } else { tex.card_face(CardValue(0)) }),
												Node {
													width: Val::Px(img_px),
													height: Val::Px(img_px),
													..default()
												},
												if val != EMPTY { Visibility::Inherited } else { Visibility::Hidden },
											));
										});
								}
							});
					}
				});

				spawn_hand(row, &snap.hands, Player::B, hide, &tex);
			});

			// Command line at the bottom
			root.spawn((
				CommandLine,
				Text::new(""),
				TextFont { font_size: 18.0, ..default() },
				TextColor(theme::TEXT_SELECTION),
				Node {
					position_type: PositionType::Absolute,
					bottom: Val::Px(16.0),
					..default()
				},
			));
		});
}

fn spawn_hand(parent: &mut ChildSpawnerCommands, hands: &[Vec<u8>; 2], player: Player, hidden: bool, tex: &Textures) {
	let hand = &hands[player.index() as usize];
	let title = format!("{}", PlayerDisplay(player));

	parent
		.spawn(Node {
			flex_direction: FlexDirection::Column,
			align_items: AlignItems::Center,
			row_gap: Val::Px(5.0),
			width: Val::Px(100.0),
			..default()
		})
		.with_children(|col| {
			col.spawn((
				Text::new(title),
				TextFont { font_size: 18.0, ..default() },
				TextColor(match player {
					Player::A => theme::TEXT_P1,
					Player::B => theme::TEXT_P2,
				}),
			));

			for v in 0..=5u8 {
				let count = if hidden { 0 } else { hand[v as usize] };
				col.spawn((
					HandCard { player, value: CardValue(v) },
					Button,
					Node {
						width: Val::Px(80.0),
						height: Val::Px(55.0),
						justify_content: JustifyContent::Center,
						align_items: AlignItems::Center,
						..default()
					},
					BackgroundColor(if hidden || count == 0 { theme::HAND_CARD_EMPTY } else { theme::HAND_CARD }),
				))
				.with_children(|card| {
					card.spawn((
						ImageNode::new(tex.card_face(CardValue(v))),
						Node {
							width: Val::Px(45.0),
							height: Val::Px(45.0),
							..default()
						},
						if hidden { Visibility::Hidden } else { Visibility::Inherited },
					));
					card.spawn((
						HandCountLabel { player, value: CardValue(v) },
						Text::new(if hidden { String::new() } else { format!("x{count}") }),
						TextFont { font_size: 14.0, ..default() },
						TextColor(if count == 0 { theme::TEXT_MUTED } else { theme::TEXT_PRIMARY }),
					));
				});
			}
		});
}

fn ai_turn(mut game: ResMut<Game>, slots: Res<PlayerSlots>) {
	if game.0.is_done() {
		return;
	}
	let turn = game.0.turn();
	if slots.0[turn.index() as usize].is_manual() {
		return;
	}
	match game.0.next(None) {
		ControlFlow::Continue(()) => debug!("AI moved"),
		ControlFlow::Break(result) => debug!("AI move ended game: {} vs {}", result.p1_score, result.p2_score),
	}
}

fn hand_click(
	mut commands: Commands,
	interaction_query: Query<(Entity, &Interaction, &HandCard), Changed<Interaction>>,
	mut selected: ResMut<SelectedCard>,
	game: Res<Game>,
	slots: Res<PlayerSlots>,
	init: Res<InitialPlayers>,
	mut modal: ResMut<ModalState<GameAction>>,
) {
	let turn = game.0.turn();
	let is_manual = slots.0[turn.index() as usize].is_manual();
	if !is_manual {
		return;
	}
	let hands = if init.hide { [game.0.p1_hand(), vec![0u8; 6]] } else { game.0.hands() };
	for (entity, interaction, hand_card) in &interaction_query {
		if *interaction != Interaction::Pressed {
			continue;
		}
		if hand_card.player != turn {
			commands.entity(entity).insert(RejectFlash(Timer::from_seconds(0.3, TimerMode::Once)));
			continue;
		}
		let count = hands[turn.index() as usize][hand_card.value.0 as usize];
		if count > 0 {
			if selected.0 == Some(hand_card.value) {
				selected.0 = None;
			} else {
				selected.0 = Some(hand_card.value);
			}
			modal.reset();
		}
	}
}

#[derive(Component)]
struct RejectFlash(Timer);

fn reject_flash_system(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut RejectFlash, &mut BackgroundColor)>) {
	for (entity, mut flash, mut bg) in &mut query {
		flash.0.tick(time.delta());
		let t = flash.0.fraction();
		let intensity = 1.0 - t;
		*bg = BackgroundColor(Color::oklcha(0.45 + 0.1 * intensity, 0.18 * intensity, 25.0, 0.7 + 0.3 * intensity));
		if flash.0.is_finished() {
			commands.entity(entity).remove::<RejectFlash>();
		}
	}
}

fn board_click(
	interaction_query: Query<(&Interaction, &BoardCell), Changed<Interaction>>,
	mut game: ResMut<Game>,
	mut selected: ResMut<SelectedCard>,
	slots: Res<PlayerSlots>,
	mut modal: ResMut<ModalState<GameAction>>,
) {
	if game.0.is_done() {
		return;
	}
	let turn = game.0.turn();
	if !&slots.0[turn.index() as usize].is_manual() {
		return;
	}
	let Some(card) = selected.0 else { return };

	for (interaction, cell) in &interaction_query {
		if *interaction == Interaction::Pressed {
			let pos = Pos { row: cell.row, col: cell.col };
			if game.0.is_playable(pos) {
				match game.0.next(Some(Move { pos, card })) {
					ControlFlow::Continue(()) => debug!("move applied"),
					ControlFlow::Break(result) => debug!("game ended: {} vs {}", result.p1_score, result.p2_score),
				}
				selected.0 = None;
				modal.reset();
				return;
			}
		}
	}
}

/// Rebuild the modal tree based on current game state.
///
/// Always includes `:q` for exit. When a card is selected on a manual player's turn,
/// also includes position keys (column letters → row digits) for all playable positions.
fn rebuild_modal_tree(game: Res<Game>, selected: Res<SelectedCard>, slots: Res<PlayerSlots>, mut modal: ResMut<ModalState<GameAction>>) {
	use v_utils::bevy::ModalNode;

	let mut root = ModalNode::<GameAction>::new();

	// :q → exit (colon then q)
	root.children.insert(
		':',
		ModalNode {
			children: [(
				'q',
				ModalNode {
					action: Some(GameAction::Exit),
					..default()
				},
			)]
			.into_iter()
			.collect(),
			label: Some("command"),
			..default()
		},
	);

	// Position shortcuts when card is selected
	if selected.0.is_some() && !game.0.is_done() {
		let turn = game.0.turn();
		if slots.0[turn.index() as usize].is_manual() {
			let n = game.0.size();

			// Group playable positions by column
			for col in 0..n {
				let col_char = (b'a' + col) as char;
				let playable_rows: Vec<u8> = (0..n).filter(|&row| game.0.is_playable(Pos { row, col })).collect();

				if playable_rows.is_empty() {
					continue;
				}

				if playable_rows.len() == 1 {
					// Single playable row in this column → terminal on the column key
					let pos = Pos { row: playable_rows[0], col };
					root.children.insert(
						col_char,
						ModalNode {
							action: Some(GameAction::PlaceCard(pos)),
							label: Some("place"),
							..default()
						},
					);
				} else {
					// Multiple rows → need second key for row
					let mut col_node = ModalNode::<GameAction>::new();
					col_node.label = Some("col");
					for row in playable_rows {
						let row_char = (b'1' + row) as char;
						col_node.children.insert(
							row_char,
							ModalNode {
								action: Some(GameAction::PlaceCard(Pos { row, col })),
								label: Some("row"),
								..default()
							},
						);
					}
					root.children.insert(col_char, col_node);
				}
			}
		}
	}

	// Only update if tree actually changed (avoid resetting active sequence unnecessarily).
	// Simple heuristic: compare child key sets. Full structural comparison not worth it.
	let old_keys: Vec<char> = modal.root.children.keys().copied().collect();
	let new_keys: Vec<char> = root.children.keys().copied().collect();
	if old_keys != new_keys {
		modal.root = root;
		// Don't reset sequence — if user is mid-`:q`, keep it alive.
		// Only reset if the active sequence is no longer valid.
		if modal.active {
			if modal.current_node().is_none() {
				modal.reset();
			}
		}
	} else {
		modal.root = root;
	}
}

/// Process keyboard input through the modal system, with descriptive warnings on invalid keys.
fn process_modal_input(
	time: Res<Time>,
	pressed_chars: Res<PressedChars>,
	mut modal: ResMut<ModalState<GameAction>>,
	mut actions: bevy::ecs::message::MessageWriter<ModalActionFired<GameAction>>,
	game: Res<Game>,
	selected: Res<SelectedCard>,
	mut warning: ResMut<Warning>,
) {
	// Tick warning timer
	if warning.timer > 0.0 {
		warning.timer -= time.delta_secs();
		if warning.timer <= 0.0 {
			warning.text.clear();
		}
	}

	// Escape resets
	if pressed_chars.logical_keys_just_pressed.contains(&KeyCode::Escape) && (modal.active || modal.show_help) {
		modal.reset();
		return;
	}

	if modal.show_help && !pressed_chars.just_pressed.is_empty() {
		modal.reset();
	}

	// Hint timeout
	if modal.active {
		modal.time_since_last_key += time.delta_secs();
		if modal.time_since_last_key >= v_utils::bevy::MODAL_HINT_TIMEOUT && !modal.hints_visible {
			modal.hints_visible = true;
		}
	}

	let n = game.0.size();

	for &key in &pressed_chars.just_pressed {
		if modal.active {
			if modal.is_valid_key(key) {
				if let Some(action) = modal.process_key(key) {
					actions.write(ModalActionFired(action));
				}
			} else {
				// Generate a descriptive warning
				let first = modal.sequence.first().copied();
				let msg = match first {
					Some(':') => format!(":{key} is not a command"),
					Some(col_ch) if col_ch.is_ascii_lowercase() => {
						let col = col_ch as u8 - b'a';
						let col_label = (b'a' + col) as char;
						if key.is_ascii_digit() {
							let row = key as u8 - b'1';
							let pos = Pos { row, col };
							if row >= n {
								format!("{col_label}{key}: row out of bounds")
							} else if !game.0.is_playable(pos) {
								format!("{col_label}{key}: not a valid placement")
							} else {
								format!("{col_label}{key}: invalid")
							}
						} else {
							format!("{col_ch}{key}: expected row number")
						}
					}
					_ => format!("'{key}' is not a valid key"),
				};
				warning.text = msg;
				warning.timer = WARNING_DURATION;
				modal.reset();
			}
		} else if modal.root.children.contains_key(&key) {
			if let Some(action) = modal.process_key(key) {
				actions.write(ModalActionFired(action));
			}
		} else if selected.0.is_some() && key.is_ascii_lowercase() {
			// User typed a column letter but it's not in the tree (no playable positions there)
			let col = key as u8 - b'a';
			if col < n {
				warning.text = format!("no playable cells in column {key}");
				warning.timer = WARNING_DURATION;
			}
		}
	}
}

/// Handle completed modal actions.
fn handle_modal_action(
	mut actions: MessageReader<ModalActionFired<GameAction>>,
	mut game: ResMut<Game>,
	mut selected: ResMut<SelectedCard>,
	mut exit: bevy::ecs::message::MessageWriter<AppExit>,
) {
	for ModalActionFired(action) in actions.read() {
		match action {
			GameAction::Exit => {
				exit.write(AppExit::Success);
			}
			GameAction::PlaceCard(pos) => {
				let Some(card) = selected.0 else { continue };
				if game.0.is_playable(*pos) {
					match game.0.next(Some(Move { pos: *pos, card })) {
						ControlFlow::Continue(()) => debug!("modal: placed at ({},{})", pos.row, pos.col),
						ControlFlow::Break(result) => debug!("game ended: {} vs {}", result.p1_score, result.p2_score),
					}
					selected.0 = None;
				}
			}
		}
	}
}

/// Show the current modal sequence or warning in the command line at the bottom.
fn sync_command_line(modal: Res<ModalState<GameAction>>, warning: Res<Warning>, mut query: Query<(&mut Text, &mut TextColor), With<CommandLine>>) {
	for (mut text, mut color) in &mut query {
		if modal.active {
			let seq: String = modal.sequence.iter().collect();
			**text = seq;
			*color = TextColor(theme::TEXT_SELECTION);
		} else if !warning.text.is_empty() {
			**text = warning.text.clone();
			let alpha = (warning.timer / WARNING_DURATION).clamp(0.0, 1.0);
			*color = TextColor(Color::oklcha(0.65, 0.18, 25.0, alpha));
		} else {
			**text = String::new();
		}
	}
}

fn sync_visuals(
	game: Res<Game>,
	selected: Res<SelectedCard>,
	slots: Res<PlayerSlots>,
	tex: Res<Textures>,
	modal: Res<ModalState<GameAction>>,
	init: Res<InitialPlayers>,
	mut board_cells: Query<(&BoardCell, &mut BackgroundColor, &Children)>,
	mut hand_counts: Query<(&HandCountLabel, &mut Text, &mut TextColor)>,
	mut hand_cards: Query<(&HandCard, &mut BackgroundColor, &Interaction, Has<RejectFlash>), Without<BoardCell>>,
	mut cell_images: Query<(&mut ImageNode, &mut Visibility)>,
	mut turn_indicator: Query<(&mut Text, &mut TextColor), (With<TurnIndicator>, Without<HandCountLabel>, Without<HandCard>, Without<CommandLine>)>,
) {
	let turn = game.0.turn();
	let hide = init.hide;
	let hands = if hide { [game.0.p1_hand(), vec![0u8; 6]] } else { game.0.hands() };

	// If user has typed a column letter, narrow highlight to that column only
	let highlight_col: Option<u8> = if modal.active && !modal.sequence.is_empty() {
		let first = modal.sequence[0];
		if first.is_ascii_lowercase() { Some(first as u8 - b'a') } else { None }
	} else {
		None
	};

	// Board cells
	for (cell, mut bg, children) in &mut board_cells {
		let value = game.0.get(Pos { row: cell.row, col: cell.col });
		let is_playable = game.0.is_playable(Pos { row: cell.row, col: cell.col });
		let is_manual = slots.0[turn.index() as usize].is_manual();
		let highlighted = selected.0.is_some()
			&& is_playable
			&& is_manual
			&& match highlight_col {
				Some(col) => cell.col == col,
				None => true,
			};

		*bg = if value != EMPTY {
			BackgroundColor(theme::CELL_OCCUPIED)
		} else if highlighted {
			BackgroundColor(theme::CELL_HIGHLIGHT)
		} else {
			BackgroundColor(theme::CELL_EMPTY)
		};

		for child in children.iter() {
			if let Ok((mut img, mut vis)) = cell_images.get_mut(child) {
				if value != EMPTY {
					img.image = tex.card_face(CardValue(value));
					*vis = Visibility::Inherited;
				} else {
					*vis = Visibility::Hidden;
				}
			}
		}
	}

	// Hand counts
	for (hc, mut text, mut color) in &mut hand_counts {
		if hide && hc.player == Player::B {
			**text = String::new();
			*color = TextColor(theme::TEXT_MUTED);
			continue;
		}
		let count = hands[hc.player.index() as usize][hc.value.0 as usize];
		**text = format!("x{count}");
		*color = if count == 0 {
			TextColor(theme::TEXT_MUTED)
		} else if selected.0 == Some(hc.value) && hc.player == turn {
			TextColor(theme::TEXT_SELECTION)
		} else {
			TextColor(theme::TEXT_PRIMARY)
		};
	}

	// Hand card backgrounds
	for (hc, mut bg, interaction, has_reject) in &mut hand_cards {
		if has_reject {
			continue;
		}
		if hide && hc.player == Player::B {
			*bg = BackgroundColor(theme::HAND_CARD_EMPTY);
			continue;
		}
		let count = hands[hc.player.index() as usize][hc.value.0 as usize];
		let is_own = hc.player == turn;
		let is_selected = selected.0 == Some(hc.value) && is_own;
		let is_hovered = *interaction == Interaction::Hovered && is_own && count > 0;

		*bg = if count == 0 {
			BackgroundColor(theme::HAND_CARD_EMPTY)
		} else if is_selected {
			BackgroundColor(theme::HAND_CARD_SELECTED)
		} else if is_hovered {
			BackgroundColor(theme::HAND_CARD_HOVER)
		} else if !is_own {
			BackgroundColor(theme::HAND_CARD_OPPONENT)
		} else {
			BackgroundColor(theme::HAND_CARD)
		};
	}

	// Turn indicator
	for (mut text, mut color) in &mut turn_indicator {
		if game.0.is_done() {
			**text = "Game Over!".into();
			*color = TextColor(theme::TEXT_GAME_OVER);
		} else {
			**text = format!("{}'s turn", PlayerDisplay(turn));
			*color = TextColor(theme::TEXT_PRIMARY);
		}
	}
}

fn check_terminal(game: Res<Game>, mut next_state: ResMut<NextState<AppState>>) {
	if game.0.is_done() {
		next_state.set(AppState::Result);
	}
}

fn keyboard_card_select(
	keys: Res<ButtonInput<KeyCode>>,
	mut selected: ResMut<SelectedCard>,
	game: Res<Game>,
	slots: Res<PlayerSlots>,
	init: Res<InitialPlayers>,
	mut modal: ResMut<ModalState<GameAction>>,
) {
	if game.0.is_done() {
		return;
	}
	// Don't handle digit keys if modal is active (user might be typing a row number)
	if modal.active {
		return;
	}
	let turn = game.0.turn();
	if !&slots.0[turn.index() as usize].is_manual() {
		return;
	}
	let pressed = if keys.just_pressed(KeyCode::Digit0) || keys.just_pressed(KeyCode::Numpad0) {
		Some(CardValue(0))
	} else if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
		Some(CardValue(1))
	} else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
		Some(CardValue(2))
	} else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
		Some(CardValue(3))
	} else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
		Some(CardValue(4))
	} else if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5) {
		Some(CardValue(5))
	} else {
		None
	};
	let Some(card) = pressed else { return };
	let hands = if init.hide { [game.0.p1_hand(), vec![0u8; 6]] } else { game.0.hands() };
	let count = hands[turn.index() as usize][card.0 as usize];
	if count > 0 {
		if selected.0 == Some(card) {
			selected.0 = None;
		} else {
			selected.0 = Some(card);
		}
		modal.reset();
	}
}

fn handle_escape(
	keys: Res<ButtonInput<KeyCode>>,
	mut selected: ResMut<SelectedCard>,
	mut next_state: ResMut<NextState<AppState>>,
	modal: Res<ModalState<GameAction>>,
	mut was_modal_active: Local<bool>,
) {
	// update_modal_state already resets on Escape, so track whether it was active
	// to avoid double-action in the same frame.
	let modal_active_now = modal.active;
	if keys.just_pressed(KeyCode::Escape) {
		if *was_modal_active {
			// Modal just got reset by update_modal_state — don't also deselect
		} else if selected.0.is_some() {
			selected.0 = None;
		} else {
			next_state.set(AppState::Menu);
		}
	}
	*was_modal_active = modal_active_now;
}

fn cleanup_gameplay(mut commands: Commands, query: Query<Entity, With<GameScene>>) {
	for entity in &query {
		commands.entity(entity).despawn();
	}
	commands.remove_resource::<SelectedCard>();
	// Game and PlayerSlots survive into Result state — cleaned up there.
}
