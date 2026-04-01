# Training Performance Analysis

## Pipeline Overview

Each training iteration is three phases:

```
Self-play (Rust) → Training (Python/PyTorch) → ONNX export (Python)
     ~90%                  ~9%                       ~1%
```

For `--iterations 300 --games 200 --sims 16`:
- 300 × (self-play 200 games + train ~20 steps + export)
- Total games: 60,000 — each ~15-20 moves long → ~1M Gumbel searches

---

## Where Time Is Spent

### Self-play: ~90%

| Sub-task | % of self-play | Location |
|---|---|---|
| **NN inference (ONNX, batch=1)** | ~65% | `nn_eval.rs` → `ort::Session::run` |
| MCTS selection/backprop | ~20% | `mcts.rs`, `gumbel.rs` |
| State clone & move generation | ~15% | `GameState::clone`, `valid_moves()` |

The dominant bottleneck is every MCTS leaf expansion issuing a **single-sample ONNX forward pass** (`[1, 33, 5, 5]`). The network is a 5-block SE-ResNet (~600k params), expensive relative to the tiny board size, and batch=1 leaves GPU/CPU throughput almost completely unutilized.

With rayon parallelism, each thread runs its own ONNX session sequentially — there is no cross-thread batching.

### Training: ~9%

PyTorch SGD over `games / 10` batches of 256 samples per iteration. Already uses GPU if available. Not the bottleneck.

### ONNX export: ~1%

`torch.onnx.export` + roundtrip validation. Negligible.

---

## Neural Network (model_resnet.py)

```
Input: [batch, 33, 5, 5]

Body:
  Conv2d(33→64, 3×3) + BN + ReLU
  5× ResBlock:
    Conv2d(64→64, 3×3) + BN + ReLU
    Conv2d(64→64, 3×3) + BN
    SE block: AvgPool → FC(64→8) → FC(8→128) → channel attention
    Residual + ReLU

Policy head: Conv2d(64→2, 1×1) + BN + ReLU → FC(50→150)  →  6·N² logits
Value head:  Conv2d(64→1, 1×1) + BN + ReLU → FC(25→64) → FC(64→1) → tanh
```

~600k parameters. Roughly 130M MACs per sample.

---

## What Was Done: CUDA EP

**Switched ORT ONNX inference from CPU to CUDA execution provider** (`nn_eval.rs`, `Cargo.toml`).

With CUDA EP, ORT dispatches the forward pass to the GPU. Even at batch=1, this removes the CPU as the bottleneck for the ResNet convolutions. Expected speedup: **2–5×** on self-play, depending on GPU and how much overhead the per-call kernel launch costs.

```rust
// nn_eval.rs
Session::builder()?
    .with_execution_providers([CUDAExecutionProvider::default().build()])?
    .commit_from_file(model_path)?
```

Requires: CUDA-enabled ORT runtime at link/run time. The `ort` crate's `cuda` feature enables the required bindings. ORT will silently fall back to CPU if no CUDA device is available at runtime (this is ORT's default behavior with EP registration).

---

## Bigger Remaining Win: Batched Evaluation

CUDA EP at batch=1 is still wasteful — GPU utilization will be low (~5–15%) because each kernel launch is for a single image. The architectural fix is **batched leaf evaluation**:

Instead of `evaluator.evaluate(leaf)` once per simulation, buffer all positions that need evaluation across the `sims_per_action × survivors` calls in a Gumbel phase and dispatch one batched ONNX call.

With batch=16–64, GPU utilization rises dramatically and throughput per call scales near-linearly. This requires restructuring `gumbel_search` to separate simulation/selection from evaluation — i.e., run selection to leaves, collect all pending `(tree_path, state)` pairs, batch-evaluate, then backpropagate.

Estimated additional speedup over CUDA+batch=1: **10–30×** on self-play.
