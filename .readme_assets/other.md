## Training (AlphaZero CNN)

One iteration of the training loop:
1. **Self-play** (Rust) — Gumbel-guided games write `(state, policy, value)` samples to `$XDG_CACHE_HOME/robot_master_train/training_data/`
2. **Train** (Python) — SE-ResNet fits on the new samples, saves checkpoint to `$XDG_CACHE_HOME/robot_master_train/models/`
3. **Export** (Python) — checkpoint → `models/model_vN.onnx` for the next self-play iteration

Run the full loop with the included script:

```sh
./scripts/train_cnn.rs
./scripts/train_cnn.rs --iterations 30 --games 300 --sims 50
```

**Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `--iterations` | `20` | Number of selfplay → train → export cycles |
| `--games` | `200` | Self-play games per iteration |
| `--sims` | `25` | Gumbel simulations per move |
| `--epochs` | `5` | Training epochs per iteration |
| `--data-dir` | `$XDG_CACHE_HOME/robot_master_train/training_data` | Where `.bin` game files are written |
| `--models-dir` | `$XDG_CACHE_HOME/robot_master_train/models` | Where checkpoints and `.onnx` files live |

**Rough timing on CPU (32-core), no GPU:**

| Target | Iterations | Wall time |
|--------|-----------|-----------|
| Beat `sadist` (~1027 Elo) | ~10 | ~3 min |
| Beat `rollout` (~1344 Elo) | ~25 | ~8 min |

The script prints iteration number, loss breakdown, and a running Elo estimate against `rollout` every 5 iterations.

**Resuming after interruption:** safe to kill and restart at any time. The current iteration is lost, but all prior `.onnx` models and accumulated training data survive. The next run picks up from the latest model automatically. Caveat: don't mix `--board-size` values across runs — 5x5 and 7x7 samples share the same data directory and have incompatible tensor shapes.
