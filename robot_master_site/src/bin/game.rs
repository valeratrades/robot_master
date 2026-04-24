use std::path::PathBuf;
fn main() {
	use robot_master_arena::{
		BoardSize,
		algos::{InnerKind, PlayerKind},
		player::ManualPlayer,
	};
	robot_master_game::create_app(
		"public",
		BoardSize::DEFAULT,
		false,
		PlayerKind {
			inner: InnerKind::ManualPlayer(ManualPlayer::default()),
			sims: None,
			constrain_sizes: None,
			constrain_hide: None,
		},
		PlayerKind {
			inner: InnerKind::RandomPlayer(Default::default()),
			sims: None,
			constrain_sizes: None,
			constrain_hide: None,
		},
		true,
		PathBuf::default(),
	)
	.run();
}
