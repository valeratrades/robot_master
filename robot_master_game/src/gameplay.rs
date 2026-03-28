use bevy::prelude::*;
use robot_master_arena::{
	algos::{greedy::GreedyPlayer, random::RandomPlayer, sadist::SadistPlayer},
	match_::Match,
	player::{ManualPlayer, Player},
};
use robot_master_core::{
	board::{EMPTY, Pos},
	cards::CardValue,
	game::{GameConfig, GameState, Move, PlayerId},
};

use crate::{AppState, InitialPlayers, PlayerKind, Textures};

type GameMatch = Match<5, Box<dyn Player<5>>, Box<dyn Player<5>>>;
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(OnEnter(AppState::Playing), setup_gameplay)
			.add_systems(
				Update,
				(ai_turn, hand_click, board_click, sync_visuals, check_terminal).chain().run_if(in_state(AppState::Playing)),
			)
			.add_systems(OnExit(AppState::Playing), cleanup_gameplay);
	}
}

#[derive(Component)]
struct GameScene;

#[derive(Resource)]
pub(crate) struct Game(pub(crate) GameMatch);

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
	player: PlayerId,
	value: CardValue,
}

#[derive(Component)]
struct HandCountLabel {
	player: PlayerId,
	value: CardValue,
}

#[derive(Component)]
struct TurnIndicator;

fn player_from_kind(kind: &PlayerKind) -> Box<dyn Player<5>> {
	match kind {
		PlayerKind::Manual { name } => Box::new(ManualPlayer::new(name)),
		PlayerKind::Random => Box::new(RandomPlayer::new()),
		PlayerKind::Greedy => Box::new(GreedyPlayer),
		PlayerKind::Sadist => Box::new(SadistPlayer),
	}
}

fn setup_gameplay(mut commands: Commands, init: Res<InitialPlayers>, tex: Res<Textures>) {
	let mut rng: rand::rngs::SmallRng = rand::make_rng();
	let game = GameState::<5>::new(GameConfig::default(), &mut rng);

	let p1_kind = PlayerKind::from_name(&init.p1);
	let p2_kind = PlayerKind::from_name(&init.p2);
	let p1 = player_from_kind(&p1_kind);
	let p2 = player_from_kind(&p2_kind);
	let m = Match::new(game, p1, p2);
	let initial_state = *m.game();

	commands.insert_resource(Game(m));
	commands.insert_resource(SelectedCard::default());
	commands.insert_resource(PlayerSlots([p1_kind, p2_kind]));

	// Root layout: [P1 hand] [Board] [P2 hand]
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
			BackgroundColor(Color::srgb(0.08, 0.08, 0.15)),
		))
		.with_children(|root| {
			// Turn indicator
			root.spawn((
				TurnIndicator,
				Text::new(""),
				TextFont { font_size: 24.0, ..default() },
				TextColor(Color::WHITE),
				Node {
					margin: UiRect::bottom(Val::Px(10.0)),
					..default()
				},
			));

			// Main row: hand - board - hand
			root.spawn(Node {
				flex_direction: FlexDirection::Row,
				align_items: AlignItems::Center,
				column_gap: Val::Px(20.0),
				..default()
			})
			.with_children(|row| {
				// Player 1 hand
				spawn_hand(row, &initial_state, PlayerId::Cols, &tex);

				// Board
				row.spawn(Node {
					flex_direction: FlexDirection::Column,
					..default()
				})
				.with_children(|board| {
					for r in 0..5u8 {
						board
							.spawn(Node {
								flex_direction: FlexDirection::Row,
								..default()
							})
							.with_children(|board_row| {
								for c in 0..5u8 {
									let val = initial_state.board.get(Pos { row: r, col: c });
									board_row
										.spawn((
											BoardCell { row: r, col: c },
											Button,
											Node {
												width: Val::Px(80.0),
												height: Val::Px(80.0),
												margin: UiRect::all(Val::Px(2.0)),
												justify_content: JustifyContent::Center,
												align_items: AlignItems::Center,
												..default()
											},
											BackgroundColor(if val != EMPTY { Color::srgba(0.3, 0.3, 0.5, 0.8) } else { Color::srgba(0.2, 0.2, 0.3, 0.4) }),
										))
										.with_children(|cell| {
											// Always spawn the ImageNode; hide it if cell is empty
											cell.spawn((
												ImageNode::new(if val != EMPTY { tex.card_face(CardValue(val)) } else { tex.card_face(CardValue(0)) }),
												Node {
													width: Val::Px(70.0),
													height: Val::Px(70.0),
													..default()
												},
												if val != EMPTY { Visibility::Inherited } else { Visibility::Hidden },
											));
										});
								}
							});
					}
				});

				// Player 2 hand
				spawn_hand(row, &initial_state, PlayerId::Rows, &tex);
			});
		});
}

fn spawn_hand(parent: &mut ChildSpawnerCommands, game: &GameState<5>, player: PlayerId, tex: &Textures) {
	let hand = &game.hands[player as usize];
	let title = match player {
		PlayerId::Cols => "P1 (Cols)",
		PlayerId::Rows => "P2 (Rows)",
	};

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
					PlayerId::Cols => Color::srgb(0.3, 0.6, 1.0),
					PlayerId::Rows => Color::srgb(1.0, 0.4, 0.4),
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
					BackgroundColor(if count == 0 { Color::srgba(0.2, 0.2, 0.2, 0.3) } else { Color::srgba(0.3, 0.3, 0.5, 0.7) }),
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
						TextColor(if count == 0 { Color::srgba(0.5, 0.5, 0.5, 0.5) } else { Color::WHITE }),
					));
				});
			}
		});
}

fn ai_turn(mut game: ResMut<Game>, slots: Res<PlayerSlots>) {
	if game.0.game().is_terminal() {
		return;
	}
	let turn = game.0.game().turn;
	if matches!(&slots.0[turn as usize], PlayerKind::Manual { .. }) {
		return;
	}
	match game.0.next(None) {
		Ok(gs) => info!("AI moved, turn now {:?}", gs.turn),
		Err(result) => info!("AI move ended game: {} vs {}", result.p1_score, result.p2_score),
	}
}

fn hand_click(interaction_query: Query<(&Interaction, &HandCard), Changed<Interaction>>, mut selected: ResMut<SelectedCard>, game: Res<Game>, slots: Res<PlayerSlots>) {
	let turn = game.0.game().turn;
	if !matches!(&slots.0[turn as usize], PlayerKind::Manual { .. }) {
		return;
	}
	for (interaction, hand_card) in &interaction_query {
		if *interaction == Interaction::Pressed && hand_card.player == turn {
			let count = game.0.game().hands[turn as usize].count(hand_card.value);
			info!("hand_click: card={} count={count} player={:?}", hand_card.value.0, hand_card.player);
			if count > 0 {
				if selected.0 == Some(hand_card.value) {
					selected.0 = None;
				} else {
					selected.0 = Some(hand_card.value);
				}
			}
		}
	}
}

fn board_click(interaction_query: Query<(&Interaction, &BoardCell), Changed<Interaction>>, mut game: ResMut<Game>, mut selected: ResMut<SelectedCard>, slots: Res<PlayerSlots>) {
	let gs = game.0.game();
	if gs.is_terminal() {
		return;
	}
	let turn = gs.turn;
	if !matches!(&slots.0[turn as usize], PlayerKind::Manual { .. }) {
		return;
	}
	let Some(card) = selected.0 else { return };

	for (interaction, cell) in &interaction_query {
		if *interaction == Interaction::Pressed {
			let pos = Pos { row: cell.row, col: cell.col };
			let playable = game.0.game().board.is_playable(pos);
			info!("board_click: ({},{}) card={} playable={playable}", cell.row, cell.col, card.0);
			if playable {
				match game.0.next(Some(Move { pos, card })) {
					Ok(gs) => info!("move applied, turn now {:?}", gs.turn),
					Err(result) => info!("game ended: {} vs {}", result.p1_score, result.p2_score),
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
	mut hand_cards: Query<(&HandCard, &mut BackgroundColor), Without<BoardCell>>,
	mut cell_images: Query<(&mut ImageNode, &mut Visibility)>,
	mut turn_indicator: Query<(&mut Text, &mut TextColor), (With<TurnIndicator>, Without<HandCountLabel>, Without<HandCard>)>,
) {
	let gs = game.0.game();

	// Board cells
	for (cell, mut bg, children) in &mut board_cells {
		let value = gs.board.get(Pos { row: cell.row, col: cell.col });
		let is_playable = gs.board.is_playable(Pos { row: cell.row, col: cell.col });
		let is_manual = matches!(&slots.0[gs.turn as usize], PlayerKind::Manual { .. });
		let highlighted = selected.0.is_some() && is_playable && is_manual;

		*bg = if value != EMPTY {
			BackgroundColor(Color::srgba(0.3, 0.3, 0.5, 0.8))
		} else if highlighted {
			BackgroundColor(Color::srgba(0.2, 0.7, 0.2, 0.5))
		} else {
			BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 0.4))
		};

		// Update image and visibility
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
		let count = gs.hands[hc.player as usize].count(hc.value);
		**text = format!("x{count}");
		*color = if count == 0 {
			TextColor(Color::srgba(0.5, 0.5, 0.5, 0.5))
		} else if selected.0 == Some(hc.value) && hc.player == gs.turn {
			TextColor(Color::srgb(1.0, 1.0, 0.0))
		} else {
			TextColor(Color::WHITE)
		};
	}

	// Hand card backgrounds
	for (hc, mut bg) in &mut hand_cards {
		let count = gs.hands[hc.player as usize].count(hc.value);
		let is_selected = selected.0 == Some(hc.value) && hc.player == gs.turn;
		*bg = if count == 0 {
			BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.3))
		} else if is_selected {
			BackgroundColor(Color::srgba(0.8, 0.8, 0.2, 0.8))
		} else {
			BackgroundColor(Color::srgba(0.3, 0.3, 0.5, 0.7))
		};
	}

	// Turn indicator
	for (mut text, mut color) in &mut turn_indicator {
		if gs.is_terminal() {
			**text = "Game Over!".into();
			*color = TextColor(Color::srgb(1.0, 0.3, 0.3));
		} else {
			let name = match gs.turn {
				PlayerId::Cols => "Player 1 (Cols)",
				PlayerId::Rows => "Player 2 (Rows)",
			};
			let kind = &slots.0[gs.turn as usize];
			**text = format!("{name} — {kind}'s turn");
			*color = TextColor(Color::WHITE);
		}
	}
}

fn check_terminal(game: Res<Game>, mut next_state: ResMut<NextState<AppState>>) {
	if game.0.game().is_terminal() {
		next_state.set(AppState::Result);
	}
}

fn cleanup_gameplay(mut commands: Commands, query: Query<Entity, With<GameScene>>) {
	for entity in &query {
		commands.entity(entity).despawn();
	}
	commands.remove_resource::<SelectedCard>();
	// Game and PlayerSlots survive into Result state — cleaned up there.
}
