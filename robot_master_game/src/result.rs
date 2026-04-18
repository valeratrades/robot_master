use bevy::prelude::*;
use robot_master_core::{board::Pos, cards::CardValue};

use crate::{
	AppState, Textures,
	gameplay::{EvalHistory, Game, PlayerSlots},
	theme,
};

pub struct ResultPlugin;

impl Plugin for ResultPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(OnEnter(AppState::Result), setup_result)
			.add_systems(Update, (play_again_button, keyboard_shortcuts, eval_graph_hover).run_if(in_state(AppState::Result)))
			.add_systems(OnExit(AppState::Result), cleanup_result);
	}
}

#[derive(Component)]
struct ResultScene;

#[derive(Component)]
struct PlayAgainButton;

#[derive(Component)]
struct EvalGraphColumn(usize);

#[derive(Component)]
struct EvalGraphTooltip;

fn setup_result(mut commands: Commands, game: Res<Game>, slots: Res<PlayerSlots>, tex: Res<Textures>, eval_history: Option<Res<EvalHistory>>) {
	let n = game.0.size() as usize;
	let (s0, i0, s1, i1) = game.0.scores();

	let p1_name = slots.0[0].to_string();
	let p2_name = slots.0[1].to_string();

	let verdict = match s0.cmp(&s1) {
		std::cmp::Ordering::Greater => format!("{p1_name} (Cols) wins!"),
		std::cmp::Ordering::Less => format!("{p2_name} (Rows) wins!"),
		std::cmp::Ordering::Equal => "Draw!".into(),
	};

	let scores = format!("{p1_name} (Cols): {s0} (weakest: col {i0})\n{p2_name} (Rows): {s1} (weakest: row {i1})");

	#[cfg(not(target_arch = "wasm32"))]
	let elo_text = format_elo(&slots, s0, s1, i0, i1);
	#[cfg(target_arch = "wasm32")]
	let elo_text = String::default();

	let cell_px = 320.0 / n as f32;
	let img_px = cell_px - 10.0;

	commands
		.spawn((
			ResultScene,
			Node {
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				flex_direction: FlexDirection::Column,
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				row_gap: Val::Px(15.0),
				..default()
			},
			BackgroundColor(theme::bg::RESULT),
		))
		.with_children(|root| {
			root.spawn((Text::new(&verdict), TextFont { font_size: 48.0, ..default() }, TextColor(theme::text::TITLE)));

			root.spawn(Node {
				flex_direction: FlexDirection::Column,
				margin: UiRect::vertical(Val::Px(10.0)),
				..default()
			})
			.with_children(|board| {
				for r in 0..n as u8 {
					board
						.spawn(Node {
							flex_direction: FlexDirection::Row,
							..default()
						})
						.with_children(|row| {
							for c in 0..n as u8 {
								let val = game.0.get(Pos { row: r, col: c });
								row.spawn((
									Node {
										width: Val::Px(cell_px),
										height: Val::Px(cell_px),
										margin: UiRect::all(Val::Px(1.0)),
										justify_content: JustifyContent::Center,
										align_items: AlignItems::Center,
										..default()
									},
									BackgroundColor(if val != robot_master_core::board::EMPTY { theme::cell::OCCUPIED } else { theme::cell::EMPTY }),
								))
								.with_children(|cell| {
									if val != robot_master_core::board::EMPTY {
										cell.spawn((
											ImageNode::new(tex.card_face(CardValue(val))),
											Node {
												width: Val::Px(img_px),
												height: Val::Px(img_px),
												..default()
											},
										));
									}
								});
							}
						});
				}
			});

			root.spawn((Text::new(&scores), TextFont { font_size: 22.0, ..default() }, TextColor(theme::text::PRIMARY)));

			if !elo_text.is_empty() {
				root.spawn((Text::new(&elo_text), TextFont { font_size: 18.0, ..default() }, TextColor(theme::text::ELO)));
			}

			if let Some(history) = eval_history.as_ref().filter(|h| h.0.len() > 1) {
				root.spawn((
					Node {
						width: Val::Px(500.0),
						height: Val::Px(80.0),
						flex_direction: FlexDirection::Row,
						overflow: Overflow::clip(),
						..default()
					},
					BackgroundColor(theme::bg::DARK),
				))
				.with_children(|bar| {
					for (i, &p1_win) in history.0.iter().enumerate() {
						bar.spawn((
							EvalGraphColumn(i),
							Button,
							Node {
								flex_grow: 1.0,
								height: Val::Percent(100.0),
								flex_direction: FlexDirection::Column,
								..default()
							},
						))
						.with_children(|col| {
							col.spawn((
								Node {
									width: Val::Percent(100.0),
									height: Val::Percent(p1_win * 100.0),
									..default()
								},
								BackgroundColor(theme::text::P1),
							));
							col.spawn((
								Node {
									width: Val::Percent(100.0),
									height: Val::Percent((1.0 - p1_win) * 100.0),
									..default()
								},
								BackgroundColor(theme::text::P2),
							));
						});
					}
				});

				root.spawn((
					EvalGraphTooltip,
					Text::new(""),
					TextFont { font_size: 13.0, ..default() },
					TextColor(theme::text::MUTED),
					Visibility::Hidden,
				));
			}

			root.spawn((
				PlayAgainButton,
				Button,
				Node {
					width: Val::Px(200.0),
					height: Val::Px(50.0),
					justify_content: JustifyContent::Center,
					align_items: AlignItems::Center,
					margin: UiRect::top(Val::Px(15.0)),
					..default()
				},
				BackgroundColor(theme::btn::PLAY_AGAIN),
			))
			.with_children(|btn| {
				btn.spawn((Text::new("Play Again"), TextFont { font_size: 28.0, ..default() }, TextColor(theme::text::PRIMARY)));
			});
		});
}

fn play_again_button(mut interaction_query: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<PlayAgainButton>)>, mut next_state: ResMut<NextState<AppState>>) {
	for (interaction, mut color) in &mut interaction_query {
		match *interaction {
			Interaction::Pressed => {
				next_state.set(AppState::Menu);
			}
			Interaction::Hovered => {
				*color = BackgroundColor(theme::btn::PLAY_AGAIN.lighter(0.1));
			}
			Interaction::None => {
				*color = BackgroundColor(theme::btn::PLAY_AGAIN);
			}
		}
	}
}

#[cfg(not(target_arch = "wasm32"))]
fn format_elo(slots: &PlayerSlots, s0: u16, s1: u16, i0: usize, i1: usize) -> String {
	use std::sync::Arc;

	use robot_master_arena::{db::JsonRatingDb, match_::MatchResult};

	let db: Arc<dyn robot_master_arena::db::RatingDb> = Arc::new(JsonRatingDb::default());
	let p1_id = slots.0[0].id();
	let p2_id = slots.0[1].id();
	let result = MatchResult::new(p1_id, p2_id, s0, s1, i0, i1, Vec::default(), Some(db));
	let u = result.commit();
	let d1 = u.p1_new.rating - u.p1_old.rating;
	let d2 = u.p2_new.rating - u.p2_old.rating;
	let sign = |d: f64| if d >= 0.0 { "+" } else { "" };
	format!(
		"Rating: {} {:.0} ({}{:.0}) | {} {:.0} ({}{:.0})",
		p1_id,
		u.p1_new.rating,
		sign(d1),
		d1,
		p2_id,
		u.p2_new.rating,
		sign(d2),
		d2
	)
}

fn keyboard_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
	if keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::KeyP) || keys.just_pressed(KeyCode::Escape) {
		next_state.set(AppState::Menu);
	}
}

fn eval_graph_hover(
	columns: Query<(&Interaction, &EvalGraphColumn), Changed<Interaction>>,
	history: Option<Res<EvalHistory>>,
	mut tooltip_q: Query<(&mut Text, &mut Visibility), With<EvalGraphTooltip>>,
) {
	let Some(history) = history else { return };

	let mut hovered: Option<usize> = None;
	for (interaction, col) in &columns {
		if *interaction == Interaction::Hovered {
			hovered = Some(col.0);
			break;
		}
	}

	for (mut text, mut vis) in &mut tooltip_q {
		match hovered {
			Some(idx) => {
				let prob = history.0[idx];
				**text = format!("Move {idx}: P1 {:.0}%", prob * 100.0);
				*vis = Visibility::Inherited;
			}
			None => {
				*vis = Visibility::Hidden;
			}
		}
	}
}

fn cleanup_result(mut commands: Commands, query: Query<Entity, With<ResultScene>>) {
	for entity in &query {
		commands.entity(entity).despawn();
	}
	commands.remove_resource::<Game>();
	commands.remove_resource::<PlayerSlots>();
	commands.remove_resource::<EvalHistory>();
}
