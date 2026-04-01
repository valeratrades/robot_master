## Usage

The main binary is `robot_master`. It takes two players (`-a`, `-b`), an optional board size (`-s`), and a subcommand for the interface.

Players: `manual`, `random`, `greedy`, `sadist`, `rollout`. MCTS wrapping: append `_N` sims to any player — `rollout_800`, `sadist_200`. Unrecognized names prompt registration as a named manual player (with Elo tracking), or fall back to `fzf` selection.

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

### Arena
Run tournaments between AI players. Ratings use Glicko-2.

```sh
robot_master arena tourney swiss 10             # all registered players, 10 Swiss brackets
robot_master arena tourney rating 200           # rating-based pairing, 200 rounds
robot_master arena tourney elimination 5        # single-elimination, 5 cycles
robot_master arena -s 'rollout,sadist' tourney swiss 10   # filter by regex
```

**Managing players:**
```sh
robot_master arena players list                 # show all players and ratings
robot_master arena players new                  # register all default variants
robot_master arena players new rollout_800      # register a specific variant
robot_master arena players reset-ratings        # reset all ratings to default
robot_master arena players nuke                 # remove players from DB entirely
```

**ONNX models in the arena** — after training, register a model then include it in tourneys:
```sh
# bare: runs policy head directly (greedy argmax, no search)
robot_master arena players new 'onnx:model_v15'

# with MCTS: wraps the policy+value head in N-sim tree search
robot_master arena players new 'onnx:model_v15_200'

# then run against other players
robot_master arena -s 'onnx:model_v15_200,rollout$,sadist' tourney swiss 20
```

Models are looked up in `./models` by default. Override with `--models-dir`.

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
