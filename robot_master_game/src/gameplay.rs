use bevy::prelude::*;
use robot_master_arena::{
	BoardSize,
	algos::PlayerKind,
	match_::{DynMatch, Match},
};
use robot_master_core::{
	board::{EMPTY, Pos},
	cards::CardValue,
	game::{GameConfig, GameState, Move, Player, PlayerDisplay},
};

use crate::{AppState, InitialPlayers, Textures, theme};

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(OnEnter(AppState::Playing), setup_gameplay)
			.add_systems(
				Update,
				(
					(
						ai_turn, hand_click, keyboard_card_select, board_click, sync_visuals, reject_flash_system, check_terminal, handle_escape,
					)
						.chain(),
					exit_hint_system,
				)
					.run_if(in_state(AppState::Playing)),
			)
			.add_systems(OnExit(AppState::Playing), cleanup_gameplay);
	}
}

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

/// Helper: create a `Box<dyn DynMatch>` for the given board size.
fn make_match(size: BoardSize, p1: PlayerKind, p2: PlayerKind) -> Box<dyn DynMatch + Send + Sync> {
	let mut rng: rand::rngs::SmallRng = rand::make_rng();
	let config = GameConfig {
		size: size.into(),
		..GameConfig::default()
	};

	macro_rules! go {
		($N:literal) => {{
			let game = GameState::<$N>::new(config, &mut rng);
			let p1 = p1.into_bot::<$N>();
			let p2 = p2.into_bot::<$N>();
			Box::new(Match::new(game, p1, p2))
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
	hands: [robot_master_core::cards::Hand; 2],
}

impl InitialBoard {
	fn get(&self, row: u8, col: u8) -> u8 {
		self.cells[row as usize * self.n + col as usize]
	}
}

fn setup_gameplay(mut commands: Commands, init: Res<InitialPlayers>, tex: Res<Textures>) {
	let size = init.size;
	let n = u8::from(size) as usize;

	let p1_kind = init.p1.clone();
	let p2_kind = init.p2.clone();

	let m = make_match(size, p1_kind.clone(), p2_kind.clone());

	// Snapshot initial state before handing ownership to the resource.
	let hands = m.hands();
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
				spawn_hand(row, &snap.hands, Player::A, &tex);

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

				spawn_hand(row, &snap.hands, Player::B, &tex);
			});
		});
}

fn spawn_hand(parent: &mut ChildSpawnerCommands, hands: &[robot_master_core::cards::Hand; 2], player: Player, tex: &Textures) {
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
				let count = hand.count(CardValue(v));
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
					BackgroundColor(if count == 0 { theme::HAND_CARD_EMPTY } else { theme::HAND_CARD }),
				))
				.with_children(|card| {
					card.spawn((
						ImageNode::new(tex.card_face(CardValue(v))),
						Node {
							width: Val::Px(45.0),
							height: Val::Px(45.0),
							..default()
						},
					));
					card.spawn((
						HandCountLabel { player, value: CardValue(v) },
						Text::new(format!("x{count}")),
						TextFont { font_size: 14.0, ..default() },
						TextColor(if count == 0 { theme::TEXT_MUTED } else { theme::TEXT_PRIMARY }),
					));
				});
			}
		});
}

fn ai_turn(mut game: ResMut<Game>, slots: Res<PlayerSlots>) {
	if game.0.is_terminal() {
		return;
	}
	let turn = game.0.turn();
	if matches!(&slots.0[turn.index() as usize], PlayerKind::Manual { .. }) {
		return;
	}
	match game.0.next(None) {
		Ok(()) => debug!("AI moved"),
		Err(result) => debug!("AI move ended game: {} vs {}", result.p1_score, result.p2_score),
	}
}

fn hand_click(
	mut commands: Commands,
	interaction_query: Query<(Entity, &Interaction, &HandCard), Changed<Interaction>>,
	mut selected: ResMut<SelectedCard>,
	game: Res<Game>,
	slots: Res<PlayerSlots>,
) {
	let turn = game.0.turn();
	let is_manual = matches!(&slots.0[turn.index() as usize], PlayerKind::Manual { .. });
	if !is_manual {
		return;
	}
	let hands = game.0.hands();
	for (entity, interaction, hand_card) in &interaction_query {
		if *interaction != Interaction::Pressed {
			continue;
		}
		if hand_card.player != turn {
			commands.entity(entity).insert(RejectFlash(Timer::from_seconds(0.3, TimerMode::Once)));
			continue;
		}
		let count = hands[turn.index() as usize].count(hand_card.value);
		debug!("hand_click: card={} count={count} player={:?}", hand_card.value.0, hand_card.player);
		if count > 0 {
			if selected.0 == Some(hand_card.value) {
				selected.0 = None;
			} else {
				selected.0 = Some(hand_card.value);
			}
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
		// Oklch red flash that fades out
		*bg = BackgroundColor(Color::oklcha(0.45 + 0.1 * intensity, 0.18 * intensity, 25.0, 0.7 + 0.3 * intensity));
		if flash.0.is_finished() {
			commands.entity(entity).remove::<RejectFlash>();
		}
	}
}

fn board_click(interaction_query: Query<(&Interaction, &BoardCell), Changed<Interaction>>, mut game: ResMut<Game>, mut selected: ResMut<SelectedCard>, slots: Res<PlayerSlots>) {
	if game.0.is_terminal() {
		return;
	}
	let turn = game.0.turn();
	if !matches!(&slots.0[turn.index() as usize], PlayerKind::Manual { .. }) {
		return;
	}
	let Some(card) = selected.0 else { return };

	for (interaction, cell) in &interaction_query {
		if *interaction == Interaction::Pressed {
			let pos = Pos { row: cell.row, col: cell.col };
			let playable = game.0.is_playable(pos);
			debug!("board_click: ({},{}) card={} playable={playable}", cell.row, cell.col, card.0);
			if playable {
				match game.0.next(Some(Move { pos, card })) {
					Ok(()) => debug!("move applied"),
					Err(result) => debug!("game ended: {} vs {}", result.p1_score, result.p2_score),
				}
				selected.0 = None;
				return;
			}
		}
	}
}

fn sync_visuals(
	game: Res<Game>,
	selected: Res<SelectedCard>,
	slots: Res<PlayerSlots>,
	tex: Res<Textures>,
	mut board_cells: Query<(&BoardCell, &mut BackgroundColor, &Children)>,
	mut hand_counts: Query<(&HandCountLabel, &mut Text, &mut TextColor)>,
	mut hand_cards: Query<(&HandCard, &mut BackgroundColor, &Interaction, Has<RejectFlash>), Without<BoardCell>>,
	mut cell_images: Query<(&mut ImageNode, &mut Visibility)>,
	mut turn_indicator: Query<(&mut Text, &mut TextColor), (With<TurnIndicator>, Without<HandCountLabel>, Without<HandCard>)>,
) {
	let turn = game.0.turn();
	let hands = game.0.hands();

	// Board cells
	for (cell, mut bg, children) in &mut board_cells {
		let value = game.0.get(Pos { row: cell.row, col: cell.col });
		let is_playable = game.0.is_playable(Pos { row: cell.row, col: cell.col });
		let is_manual = matches!(&slots.0[turn.index() as usize], PlayerKind::Manual { .. });
		let highlighted = selected.0.is_some() && is_playable && is_manual;

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
		let count = hands[hc.player.index() as usize].count(hc.value);
		**text = format!("x{count}");
		*color = if count == 0 {
			TextColor(theme::TEXT_MUTED)
		} else if selected.0 == Some(hc.value) && hc.player == turn {
			TextColor(theme::TEXT_SELECTION)
		} else {
			TextColor(theme::TEXT_PRIMARY)
		};
	}

	// Hand card backgrounds: tint, hover glow, selection
	for (hc, mut bg, interaction, has_reject) in &mut hand_cards {
		if has_reject {
			continue;
		}
		let count = hands[hc.player.index() as usize].count(hc.value);
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
		if game.0.is_terminal() {
			**text = "Game Over!".into();
			*color = TextColor(theme::TEXT_GAME_OVER);
		} else {
			**text = format!("{}'s turn", PlayerDisplay(turn));
			*color = TextColor(theme::TEXT_PRIMARY);
		}
	}
}

fn check_terminal(game: Res<Game>, mut next_state: ResMut<NextState<AppState>>) {
	if game.0.is_terminal() {
		next_state.set(AppState::Result);
	}
}

fn keyboard_card_select(keys: Res<ButtonInput<KeyCode>>, mut selected: ResMut<SelectedCard>, game: Res<Game>, slots: Res<PlayerSlots>) {
	if game.0.is_terminal() {
		return;
	}
	let turn = game.0.turn();
	if !matches!(&slots.0[turn.index() as usize], PlayerKind::Manual { .. }) {
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
	let hands = game.0.hands();
	let count = hands[turn.index() as usize].count(card);
	if count > 0 {
		if selected.0 == Some(card) {
			selected.0 = None;
		} else {
			selected.0 = Some(card);
		}
	}
}

/// Floating hint that fades out, shown when Escape is pressed.
#[derive(Component)]
struct ExitHint(Timer);

fn handle_escape(
	keys: Res<ButtonInput<KeyCode>>,
	mut selected: ResMut<SelectedCard>,
	mut next_state: ResMut<NextState<AppState>>,
	mut commands: Commands,
	scene: Query<Entity, With<GameScene>>,
	existing_hints: Query<Entity, With<ExitHint>>,
) {
	if keys.just_pressed(KeyCode::Escape) {
		if selected.0.is_some() {
			selected.0 = None;
		} else {
			next_state.set(AppState::Menu);
		}
		// Show hint regardless — even when going back, the flash is harmless
		for e in &existing_hints {
			commands.entity(e).despawn();
		}
		if let Ok(scene) = scene.single() {
			commands.entity(scene).with_children(|root| {
				root.spawn((
					ExitHint(Timer::from_seconds(2.0, TimerMode::Once)),
					Text::new("Ctrl+C or :q to quit"),
					TextFont { font_size: 14.0, ..default() },
					TextColor(theme::TEXT_MUTED),
					Node {
						position_type: PositionType::Absolute,
						bottom: Val::Px(12.0),
						..default()
					},
				));
			});
		}
	}
}

fn exit_hint_system(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut ExitHint, &mut TextColor)>) {
	for (entity, mut hint, mut color) in &mut query {
		hint.0.tick(time.delta());
		let alpha = 1.0 - hint.0.fraction();
		*color = TextColor(Color::oklcha(0.50, 0.0, 0.0, 0.5 * alpha));
		if hint.0.is_finished() {
			commands.entity(entity).despawn();
		}
	}
}

fn cleanup_gameplay(mut commands: Commands, query: Query<Entity, With<GameScene>>) {
	for entity in &query {
		commands.entity(entity).despawn();
	}
	commands.remove_resource::<SelectedCard>();
	// Game and PlayerSlots survive into Result state — cleaned up there.
}
