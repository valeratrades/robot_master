use robot_master_arena::{
	algos::{InnerKind, PlayerKind, SearchKind},
	player::Bot,
};

use crate::{
	gumbel::GumbelMcts,
	mcts::{RolloutEval, SearchBot, VanillaMcts},
	nn_eval::NnEval,
};

/// Construct a `Bot<N>` from a `PlayerKind`.
///
/// `models_dir` is used to resolve `.onnx` paths for `OnnxPlayer`.
/// Returns `Err` if an ONNX model fails to load.
// Structurally impossible to move onto PlayerKind: this fn needs NnEval/VanillaMcts/GumbelMcts
// from this crate, but PlayerKind lives in robot_master_arena which cannot depend on robot_master_train.
pub fn kind_into_bot<const N: usize>(kind: &PlayerKind, models_dir: &std::path::Path) -> Result<Box<dyn Bot<N>>, String>
where
	[(); N * N]:,
	[(); N + 1]:, {
	if let InnerKind::OnnxPlayer(p) = &kind.inner {
		let path = models_dir.join(format!("{}.onnx", p.stem));
		let evaluator = NnEval::new(path.to_str().ok_or_else(|| format!("non-UTF8 model path: {path:?}"))?, N, false).map_err(|e| format!("failed to load {path:?}: {e}"))?;
		return Ok(match kind.sims {
			None => Box::new(evaluator),
			Some((SearchKind::Vanilla, sims)) => Box::new(VanillaMcts::with_sims(evaluator, sims)),
			Some((SearchKind::Gumbel, sims)) => Box::new(GumbelMcts::with_sims(evaluator, sims)),
		});
	}
	if let Some((search, sims)) = kind.sims {
		fn make<S, B, const N: usize>(bot: B, sims: u32) -> Box<dyn Bot<N>>
		where
			S: SearchBot<RolloutEval<B>, N> + 'static,
			B: 'static,
			[(); N * N]:,
			[(); N + 1]:, {
			Box::new(S::with_sims(RolloutEval::new(bot), sims))
		}
		return Ok(match search {
			SearchKind::Vanilla => match &kind.inner {
				InnerKind::RandomPlayer(p) => make::<VanillaMcts<_>, _, N>(p.clone(), sims),
				InnerKind::GreedyForNumber(p) => make::<VanillaMcts<_>, _, N>(p.clone(), sims),
				InnerKind::GreedyForScocre(p) => make::<VanillaMcts<_>, _, N>(p.clone(), sims),
				InnerKind::Sadist(p) => make::<VanillaMcts<_>, _, N>(p.clone(), sims),
				InnerKind::Rollout(p) => make::<VanillaMcts<_>, _, N>(p.clone(), sims),
				InnerKind::ManualPlayer(_) => panic!("cannot wrap ManualPlayer in search"),
				InnerKind::OnnxPlayer(_) => unreachable!(),
			},
			SearchKind::Gumbel => match &kind.inner {
				InnerKind::RandomPlayer(p) => make::<GumbelMcts<_>, _, N>(p.clone(), sims),
				InnerKind::GreedyForNumber(p) => make::<GumbelMcts<_>, _, N>(p.clone(), sims),
				InnerKind::GreedyForScocre(p) => make::<GumbelMcts<_>, _, N>(p.clone(), sims),
				InnerKind::Sadist(p) => make::<GumbelMcts<_>, _, N>(p.clone(), sims),
				InnerKind::Rollout(p) => make::<GumbelMcts<_>, _, N>(p.clone(), sims),
				InnerKind::ManualPlayer(_) => panic!("cannot wrap ManualPlayer in search"),
				InnerKind::OnnxPlayer(_) => unreachable!(),
			},
		});
	}
	Ok(kind.clone().into_bot())
}
