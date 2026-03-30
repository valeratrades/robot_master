use bevy::prelude::*;
use robot_master_core::{board::Pos, cards::CardValue};

use crate::{
	AppState, Textures,
	gameplay::{Game, PlayerSlots},
	theme,
};

pub struct ResultPlugin;

impl Plugin for ResultPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(OnEnter(AppState::Result), setup_result)
			.add_systems(Update, play_again_button.run_if(in_state(AppState::Result)))
			.add_systems(OnExit(AppState::Result), cleanup_result);
	}
}

#[derive(Component)]
struct ResultScene;

#[derive(Component)]
struct PlayAgainButton;

fn setup_result(mut commands: Commands, game: Res<Game>, slots: Res<PlayerSlots>, tex: Res<Textures>) {
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

	let elo_text = format_elo(&slots, s0, s1, i0, i1);

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
			BackgroundColor(theme::BG_RESULT),
		))
		.with_children(|root| {
			root.spawn((Text::new(&verdict), TextFont { font_size: 48.0, ..default() }, TextColor(theme::TEXT_TITLE)));

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
									BackgroundColor(if val != robot_master_core::board::EMPTY { theme::CELL_OCCUPIED } else { theme::CELL_EMPTY }),
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

			root.spawn((Text::new(&scores), TextFont { font_size: 22.0, ..default() }, TextColor(theme::TEXT_PRIMARY)));

			if !elo_text.is_empty() {
				root.spawn((Text::new(&elo_text), TextFont { font_size: 18.0, ..default() }, TextColor(theme::TEXT_ELO)));
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
				BackgroundColor(theme::BTN_PLAY_AGAIN),
			))
			.with_children(|btn| {
				btn.spawn((Text::new("Play Again"), TextFont { font_size: 28.0, ..default() }, TextColor(theme::TEXT_PRIMARY)));
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
				*color = BackgroundColor(theme::BTN_PLAY_AGAIN_HOVER);
			}
			Interaction::None => {
				*color = BackgroundColor(theme::BTN_PLAY_AGAIN);
			}
		}
	}
}

fn format_elo(slots: &PlayerSlots, s0: u16, s1: u16, i0: usize, i1: usize) -> String {
	#[cfg(not(target_arch = "wasm32"))]
	{
		use robot_master_arena::{db::JsonRatingDb, match_::MatchResult};

		#[allow(deprecated)]
		let db = JsonRatingDb::new();
		let mut result = MatchResult {
			p1_id: slots.0[0].id(),
			p2_id: slots.0[1].id(),
			p1_score: s0,
			p2_score: s1,
			p1_weak_line: i0,
			p2_weak_line: i1,
			moves: Vec::new(),
			elo_update: None,
		};
		result.update_elo(&db);
		match result.elo_update {
			Some(ref elo) => {
				let d1 = elo.p1_new - elo.p1_old;
				let d2 = elo.p2_new - elo.p2_old;
				let sign = |d: f64| if d >= 0.0 { "+" } else { "" };
				format!(
					"Elo: {} {:.0} ({}{:.0}) | {} {:.0} ({}{:.0})",
					result.p1_id,
					elo.p1_new,
					sign(d1),
					d1,
					result.p2_id,
					elo.p2_new,
					sign(d2),
					d2
				)
			}
			None => String::new(),
		}
	}
	#[cfg(target_arch = "wasm32")]
	String::new()
}

fn cleanup_result(mut commands: Commands, query: Query<Entity, With<ResultScene>>) {
	for entity in &query {
		commands.entity(entity).despawn();
	}
	commands.remove_resource::<Game>();
	commands.remove_resource::<PlayerSlots>();
}
