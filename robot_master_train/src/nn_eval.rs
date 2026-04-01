use std::sync::Mutex;

use ort::{inputs, session::Session, value::TensorRef};
use robot_master_arena::player::Bot;
use robot_master_core::game::{GameState, Move};

use crate::{
	encoding::{action_index, encode_planes},
	mcts::{Evaluation, Evaluator},
};

/// ONNX-backed evaluator. Loads a model once; evaluate() is called per search leaf.
///
/// The ONNX model interface (see training/export_onnx.py):
///   input  "state":  f32[batch, 33, N, N]
///   output "policy": f32[batch, 6*N²]  — raw logits
///   output "value":  f32[batch]         — tanh output in [-1, 1]
pub struct NnEval {
	session: Mutex<Session>,
	board_size: usize,
}

impl NnEval {
	pub fn new(model_path: &str, board_size: usize) -> ort::Result<Self> {
		let session = Session::builder()?.commit_from_file(model_path)?;
		Ok(Self {
			session: Mutex::new(session),
			board_size,
		})
	}
}

impl<const N: usize> Evaluator<N> for NnEval
where
	[(); N * N]:,
{
	fn evaluate(&self, state: &GameState<N>) -> Evaluation {
		assert_eq!(N, self.board_size, "NnEval board_size mismatch: model={}, state={N}", self.board_size);

		let planes = encode_planes(state);
		let shape = [1usize, 33, N, N];
		let input = TensorRef::from_array_view((shape, planes.as_slice())).expect("tensor construction");

		let mut session = self.session.lock().expect("session mutex poisoned");
		let outputs = session.run(inputs!["state" => input]).expect("ort inference");

		let (_, policy_logits) = outputs["policy"].try_extract_tensor::<f32>().expect("policy extraction");
		let (_, value_raw) = outputs["value"].try_extract_tensor::<f32>().expect("value extraction");

		let value = value_raw[0];
		let policy = extract_legal_policy(policy_logits, state);
		Evaluation { policy, value }
	}
}

impl<const N: usize> Bot<N> for NnEval
where
	[(); N * N]:,
{
	fn choose_move(&mut self, state: &GameState<N>) -> Move {
		let eval = self.evaluate(state);
		eval.policy.into_iter().max_by(|a, b| a.1.partial_cmp(&b.1).expect("NaN in policy")).expect("no legal moves").0
	}
}

fn extract_legal_policy<const N: usize>(logits: &[f32], state: &GameState<N>) -> Vec<(Move, f32)>
where
	[(); N * N]:, {
	let legal: Vec<(Move, f32)> = state
		.valid_moves()
		.map(|mv| {
			let idx = action_index(mv.card.0, mv.pos.row as usize, mv.pos.col as usize, N);
			(mv, logits[idx])
		})
		.collect();

	// stable softmax over legal logits only
	let max_logit = legal.iter().map(|(_, l)| *l).fold(f32::NEG_INFINITY, f32::max);
	let exps: Vec<f32> = legal.iter().map(|(_, l)| (l - max_logit).exp()).collect();
	let sum: f32 = exps.iter().sum();
	legal.into_iter().zip(exps).map(|((mv, _), e)| (mv, e / sum)).collect()
}
