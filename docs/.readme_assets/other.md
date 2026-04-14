## Training

One iteration of the training loop:
1. **Self-play** (Rust, parallel via rayon) - Gumbel AlphaZero games write `(state, policy, value)` samples to `$XDG_CACHE_HOME/robot_master_train/<generation>/training_data/`
2. **Train** (Python) - model fits on the replay buffer, saves checkpoint to `$XDG_CACHE_HOME/robot_master_train/<generation>/models/`. Optimizer state is carried forward across iterations (SGD momentum preserved).
3. **Export** (Python) - latest checkpoint → `models/model_vN.onnx` for the next self-play iteration

```sh
robot_master train transformer --iterations 100 --games 400 --sims 25
robot_master train cnn --supervise 'rollout|v50'
```

**Options:**

| Flag | Description |
|------|-------------|
| `--exact-generation` | Include git hash in run ID (pins to exact build; fragments cache across commits) |
| `--iterations` | Number of selfplay → train → export cycles |
| `--games` | Self-play games per iteration |
| `--sims` | Gumbel simulations per move (MiniZero benchmarks n=2 and n=16) |
| `--size` | Board size (must match model architecture) |
| `--hide` | Train in information-hidden mode (opponent's hand not visible) |
| `--force-cpu` | Skip GPU for selfplay (faster at 5×5/7×7) |
| `--supervise <spec>` | *(CNN only)* Bootstrap from a rule-based bot until the NN wins >68% of eval games |

Training steps per iteration are derived automatically as `max(games/2, 1)`.

Data lives under `$XDG_CACHE_HOME/robot_master_train/<generation>/`. The run path also encodes game count, sims, board size, and hide mode - different configurations are fully isolated and safe to run in parallel.

**Replay buffer:** automatically set to the most recent `3 * ceil(ln(iterations))` iteration files (~9 for 20 iters, ~15 for 100, ~18 for 300). See `docs/references/replay_buffer_sizing.md` for rationale.

**Algorithm:** Gumbel AlphaZero (Danihelka et al., ICLR 2022) with estimated Q for unvisited nodes in UCT (MiniZero §III-B, arxiv 2310.11305). No Dirichlet noise - exploration comes from Gumbel sampling.

**Resuming after interruption:** safe to kill and restart at any time. The current iteration is lost, but all prior `.onnx` models, training data, and optimizer state survive. The next run resumes from the latest model and checkpoint automatically.
