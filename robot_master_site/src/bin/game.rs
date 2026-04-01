fn main() {
	use robot_master_arena::{
		BoardSize,
		algos::{InnerKind, PlayerKind},
		player::ManualPlayer,
	};
	robot_master_game::create_app(
		"public",
		BoardSize::DEFAULT,
		PlayerKind {
			inner: InnerKind::ManualPlayer(ManualPlayer::default()),
			sims: None,
		},
		PlayerKind {
			inner: InnerKind::RandomPlayer(Default::default()),
			sims: None,
		},
		true,
	)
	.run();
}
