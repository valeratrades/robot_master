## Usage

The main binary is `robot_master`. It takes two players (`-a`, `-b`), an optional board size (`-s`), and a subcommand for the interface.

Players: `manual` (`m`), `random` (`r`), `greedy` (`g`), `sadist` (`s`). Unrecognized names prompt registration as a named manual player (with Elo tracking), or fall back to `fzf` selection.

Board sizes: `5` (default), `7`, `9`, `11`.

### TUI
```sh
robot_master tui                            # you vs random AI, 5x5
robot_master tui -a greedy -b sadist -s 7   # watch two AIs fight on 7x7
robot_master tui -a Alice -b Bob            # two named humans, Elo tracked
```
In manual mode, the TUI prompts for card, row, column each turn. Invalid moves get a warning and re-prompt.

### GUI
```sh
robot_master gui
robot_master gui -a manual -b greedy
```
Bevy app with a main menu where you can pick players and board size from dropdowns before starting. Elo ratings are shown next to player names.

### Python
For running the project as pure Python (e.g. for grading), the Rust binary must be compiled first (`cargo b -p robot_master`). The Python modules in `py_src/` shell out to it.

```sh
python -m py_src guided -m   # partie guidée, manual (both players)
python -m py_src guided -r   # partie guidée, random (both players)
python -m py_src naive -g    # IA mode, greedy vs greedy
python -m py_src naive -a    # IA mode, sadist vs sadist
```

### Elo
Player ratings persist across games in `$XDG_DATA_HOME/robot_master/ratings.json`. Every named player (manual or AI) accumulates an Elo score. End-of-game output shows rating changes.

### Training (AlphaZero CNN)

One iteration of the training loop:
1. **Self-play** (Rust) — MCTS-guided games write `(state, policy, value)` samples to `$XDG_CACHE_HOME/robot_master_train/training_data/`
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
| `--sims` | `25` | MCTS simulations per move |
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
