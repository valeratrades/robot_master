use std::sync::Mutex;

use ort::{ep, inputs, session::Session, value::TensorRef};
use robot_master_arena::player::{Bot, StateEval};
use robot_master_core::game::{GameState, Move, Player};

use crate::{
	encoding::{action_index, encode_planes, in_channels},
	mcts::{Evaluation, Evaluator},
};

/// ONNX-backed evaluator. Loads a model once; evaluate() is called per search leaf.
///
/// The ONNX model interface (see training/export_onnx.py):
///   input  "state":  f32[batch, 33, N, N]
///   output "policy": f32[batch, 6*N²]  - raw logits
///   output "value":  f32[batch, 3]      - raw WDL logits (Win, Draw, Loss)
pub struct NnEval {
	session: Mutex<Session>,
	board_size: usize,
}

impl NnEval {
	pub fn try_new(model_path: &str, board_size: usize, force_cpu: bool) -> ort::Result<Self> {
		let mut builder = Session::builder()?;
		if !force_cpu {
			builder = builder.with_execution_providers([ep::CUDA::default().build()])?;
		}
		let session = builder.commit_from_file(model_path)?;
		let this = Self {
			session: Mutex::new(session),
			board_size,
		};
		// Flush lazy CUDA/ORT initialization so it doesn't pollute the first game's timing.
		this.warmup();
		Ok(this)
	}

	fn warmup(&self) {
		let n = self.board_size;
		let dummy: Vec<f32> = vec![0.0; in_channels(n) * n * n];
		let shape = [1usize, in_channels(n), n, n];
		let input = TensorRef::from_array_view((shape, dummy.as_slice())).expect("warmup tensor");
		let mut session = self.session.lock().expect("session mutex poisoned");
		let _ = session.run(inputs!["state" => input]);
	}
}

impl<const N: usize> Evaluator<N> for NnEval
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn evaluate(&self, state: &GameState<N>) -> Evaluation {
		self.evaluate_batch(std::slice::from_ref(state)).into_iter().next().expect("batch of 1 must return 1 result")
	}

	fn evaluate_batch(&self, states: &[GameState<N>]) -> Vec<Evaluation> {
		assert_eq!(N, self.board_size, "NnEval board_size mismatch: model={}, state={N}", self.board_size);
		let batch = states.len();

		// Encode all states into one contiguous [batch, in_channels(N), N, N] f32 buffer
		let planes_per_state = in_channels(N) * N * N;
		let mut input_buf = Vec::with_capacity(batch * planes_per_state);
		for state in states {
			input_buf.extend_from_slice(&encode_planes(state));
		}

		let shape = [batch, in_channels(N), N, N];
		let input = TensorRef::from_array_view((shape, input_buf.as_slice())).expect("tensor construction");

		let mut session = self.session.lock().expect("session mutex poisoned");
		let outputs = session.run(inputs!["state" => input]).expect("ort inference");

		let (_, policy_logits) = outputs["policy"].try_extract_tensor::<f32>().expect("policy extraction");
		let (_, value_raw) = outputs["value"].try_extract_tensor::<f32>().expect("value extraction");

		let policy_stride = policy_logits.len() / batch;
		// value_raw is [batch * 3] in WDL order: (Win, Draw, Loss) logits per sample
		states
			.iter()
			.enumerate()
			.map(|(i, state)| {
				let logits = &policy_logits[i * policy_stride..(i + 1) * policy_stride];
				let wdl_logits = &value_raw[i * 3..(i + 1) * 3];
				let value = wdl_to_scalar(wdl_logits);
				let policy = extract_legal_policy(logits, state);
				Evaluation { policy, value }
			})
			.collect()
	}
}

impl<const N: usize> Bot<N> for NnEval
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn choose_move(&mut self, state: &GameState<N>) -> Move {
		let eval = self.evaluate(state);
		eval.policy.into_iter().max_by(|a, b| a.1.partial_cmp(&b.1).expect("NaN in policy")).expect("no legal moves").0
	}
}

impl<const N: usize> StateEval<N> for NnEval
where
	[(); N * N]:,
	[(); N + 1]:,
{
	fn eval(&self, state: &GameState<N>) -> f32 {
		self.evaluate(state).value
	}
}

/// Softmax over 3 WDL logits, then return win_prob - loss_prob ∈ [-1, 1].
fn wdl_to_scalar(logits: &[f32]) -> f32 {
	let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
	let exps = [(logits[0] - max).exp(), (logits[1] - max).exp(), (logits[2] - max).exp()];
	let sum = exps[0] + exps[1] + exps[2];
	exps[0] / sum - exps[2] / sum
}

fn extract_legal_policy<const N: usize>(logits: &[f32], state: &GameState<N>) -> Vec<(Move, f32)>
where
	[(); N * N]:,
	[(); N + 1]:, {
	let legal: Vec<(Move, f32)> = state
		.valid_moves()
		.map(|mv| {
			let idx = action_index(mv.card.0, mv.pos.row as usize, mv.pos.col as usize, N, state.turn == Player::B);
			(mv, logits[idx])
		})
		.collect();

	// stable softmax over legal logits only
	let max_logit = legal.iter().map(|(_, l)| *l).fold(f32::NEG_INFINITY, f32::max);
	let exps: Vec<f32> = legal.iter().map(|(_, l)| (l - max_logit).exp()).collect();
	let sum: f32 = exps.iter().sum();
	legal.into_iter().zip(exps).map(|((mv, _), e)| (mv, e / sum)).collect()
}
