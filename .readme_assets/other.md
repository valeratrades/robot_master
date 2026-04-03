## Training (AlphaZero CNN)

One iteration of the training loop:
1. **Self-play** (Rust, parallel via rayon) — Gumbel AlphaZero games write `(state, policy, value)` samples to `$XDG_CACHE_HOME/robot_master_train/<generation>/training_data/`
2. **Train** (Python) — SE-ResNet fits on the replay buffer, saves checkpoint to `$XDG_CACHE_HOME/robot_master_train/<generation>/models/`. Optimizer state is carried forward across iterations (SGD momentum preserved).
3. **Export** (Python) — latest checkpoint → `models/model_vN.onnx` for the next self-play iteration

Run the full loop:

```sh
# quick smoke-test (few minutes on CPU)
./scripts/train_cnn.rs v1

# recommended first real run
./scripts/train_cnn.rs v1 --iterations 100 --games 200 --sims 16

# longer run
./scripts/train_cnn.rs v1 --iterations 300 --games 200 --sims 16
```

**Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `generation` | *(required)* | Label scoping all data/checkpoints/models (e.g. `v1`, `cnn_big`) |
| `--iterations` | `20` | Number of selfplay → train → export cycles |
| `--games` | `200` | Self-play games per iteration |
| `--sims` | `25` | Gumbel simulations per move (MiniZero benchmarks n=2 and n=16) |
| `--epochs` | `5` | Training epochs per iteration |

Data lives under `$XDG_CACHE_HOME/robot_master_train/<generation>/`. Different generations are fully isolated — safe to run in parallel.

**Replay buffer:** automatically set to the most recent `3 * ceil(ln(iterations))` iteration files (~9 for 20 iters, ~15 for 100, ~18 for 300). See `docs/references/replay_buffer_sizing.md` for rationale.

**Algorithm:** Gumbel AlphaZero (Danihelka et al., ICLR 2022) with estimated Q for unvisited nodes in UCT (MiniZero §III-B, arxiv 2310.11305). No Dirichlet noise — exploration comes from Gumbel sampling.

**Resuming after interruption:** safe to kill and restart at any time. The current iteration is lost, but all prior `.onnx` models, training data, and optimizer state survive. The next run resumes from the latest model and checkpoint automatically.
