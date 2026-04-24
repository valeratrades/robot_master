# robot_master <img width="25%" src="https://www.jeuxdenim.be/images/jeux/RobotMaster_large01.jpg" alt="Robot Master">
![Minimum Supported Rust Version](https://img.shields.io/badge/nightly-1.92+-ab6000.svg)
[<img alt="crates.io" src="https://img.shields.io/crates/v/robot_master.svg?color=fc8d62&logo=rust" height="20" style=flat-square>](https://crates.io/crates/robot_master)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs&style=flat-square" height="20">](https://docs.rs/robot_master)
![Lines Of Code](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/robot_master-loc.json)
<br>
[<img alt="ci errors" src="https://img.shields.io/github/actions/workflow/status/valeratrades/robot_master/errors.yml?branch=master&style=for-the-badge&style=flat-square&label=errors&labelColor=420d09" height="20">](https://github.com/valeratrades/robot_master/actions?query=branch%3Amaster) <!--NB: Won't find it if repo is private-->
[<img alt="ci warnings" src="https://img.shields.io/github/actions/workflow/status/valeratrades/robot_master/warnings.yml?branch=master&style=for-the-badge&style=flat-square&label=warnings&labelColor=d16002" height="20">](https://github.com/valeratrades/robot_master/actions?query=branch%3Amaster) <!--NB: Won't find it if repo is private-->

multi-player implementation of robot_master // in rust, because of course it is


### Reqs and `py_src/`
provisioned pdf with requirements: ./docs/.readme_assets/assets/Sujet-RobotMaster-version-04-02.pdf

rough arch outline, functionality of each function, tests, desired behavior, - can be found in this pdf file

## Rules
1v1 on a 5x5 grid. Cards are numbered 0-5, with 6 copies each (36 total). Each player gets 12; a 25th card is placed at the center of the board.

**Turns**: players alternate placing a card from their hand onto an empty cell adjacent (no diagonals) to an occupied one.

**Scoring** (per line/column, once the grid is full):
| copies of a card | points |
|---|---|
| 1 | face value (0, 1, 2, 3, 4, or 5) |
| 2 | 10 × face value (0, 10, 20, 30, 40, or 50) |
| 3+ | 100 flat, regardless of face value |

**Winner**: Alice's score = her lowest-scoring column; Bob's score = his lowest-scoring row. Highest score wins.
<!-- markdownlint-disable -->
<details>
<summary>
<h2>Installation</h2>
</summary>

### Installation
#### With Nix (recommended)
```sh
nix develop
```
This sets up Rust nightly, Python 3.12, maturin, cargo-leptos, native libraries (Vulkan, Wayland, X11, ALSA), and a Python virtualenv with all dependencies.

Then build the Rust binary and Python bindings:
```sh
cargo b -p robot_master
maturin develop --features python
```
or simply
```sh
nix build
```

For AlphaZero training, also install the training deps:
```sh
uv_sync  # alias for: uv sync --prerelease=allow --no-install-project --dev
uv sync --group train
```

#### Without Nix (but mb don't)
NB: not actually tested, - you're on your own here

##### Requirements
- Rust nightly (1.92+)
- Python >= 3.12
- System libraries: `alsa-lib`, `udev`, `vulkan-loader`, `libxkbcommon`, `wayland` (+ X11 libs if on X11)
- [`maturin`](https://github.com/PyO3/maturin) (`pip install maturin`)
- [`fzf`](https://github.com/junegunn/fzf) (optional, for player name selection in TUI)

##### Steps
```sh
# build the main binary
cargo b -p robot_master

# install python dependencies (core)
pip install typeguard icecream
# (dev: pip install pytest ruff inline-snapshot)

# training dependencies (torch, onnx, tensorboard)
pip install torch numpy onnx onnxruntime tensorboard

# build python bindings (required for `python -m py_src` to work)
maturin develop --features python
```

</details>
<!-- markdownlint-restore -->

## Usage
### Usage

The main binary is `robot_master`. It takes two players (`-a`, `-b`), an optional board size (`-s`), and a subcommand for the interface.

Players: `manual`, `random`, `greedy`, `sadist`, `rollout`. Search wrapping: append `|v<N>` (vanilla UCT-MCTS) or `|g<N>` (Gumbel) sims - `rollout|v800`, `rollout|g800`, `sadist|v200`. Unrecognized names prompt registration as a named manual player (with Elo tracking), or fall back to `fzf` selection.

Board sizes: `5`, `7`, `9`, `11`.

`--hide`: hide opponent's hand (information-hidden mode). At most one player may be manual when `--hide` is set.

#### GUI
```sh
robot_master gui
robot_master gui -a manual -b greedy
robot_master gui --sound                     # enable music and sound effects
```
Bevy app with a main menu where you can pick players and board size from dropdowns before starting. Elo ratings are shown next to player names.

<!-- markdownlint-disable -->
<details>
<summary>
<h3>If you want TUI</h3>
</summary>

```sh
robot_master tui                              # you vs random AI, 5x5
robot_master tui -a greedy -b sadist -s 7    # watch two AIs fight on 7x7
robot_master tui -a Alice -b Bob             # two named humans, Elo tracked
robot_master tui --hide                      # hidden-hand mode
```
In manual mode, the TUI prompts for card, row, column each turn. Invalid moves get a warning and re-prompt.

</details>
<!-- markdownlint-restore -->

#### Arena
Run tournaments between AI players. Ratings use Glicko-2.

```sh
robot_master arena tourney swiss 10              # all registered players, 10 Swiss brackets
robot_master arena tourney rating 200            # rating-based pairing, 200 rounds
robot_master arena tourney elimination 5         # single-elimination, 5 cycles
robot_master arena tourney round-robin 3         # every player vs every other, 3 sweeps
robot_master arena -s 'rollout,sadist' tourney swiss 10    # filter players by regex
robot_master arena tourney --json swiss 10       # output results as JSON to stdout
```

All tourney modes accept `-t <N>` / `--threads <N>`.

**Ephemeral tournaments (no ratings DB):**
```sh
# run a one-off match between specific specs without touching saved ratings
robot_master arena --no-priors 'rollout|v50,onnx:model_v15|g200' tourney swiss 20
```
`--no-priors` accepts a comma-separated list of player specs and bypasses the ratings database entirely. Mutually exclusive with `--select`.

**Managing players:**
```sh
robot_master arena players list                  # show all players and ratings
robot_master arena players new                   # register all default variants
robot_master arena players new rollout|v800      # register a specific variant
robot_master arena players reset-ratings         # reset all ratings to default
robot_master arena players nuke                  # remove players from DB entirely
```

**ONNX models in the arena** - after training, register a model then include it in tourneys:
```sh
# bare: runs policy head directly (greedy argmax, no search)
robot_master arena players new 'onnx:model_v15'

# with Gumbel search
robot_master arena players new 'onnx:model_v15|g200'

# constrain to specific board size and hide mode (required for onnx bots)
robot_master arena players new 'onnx:model_v15|g200' --sizes 5 --hide true

# then run against other players
robot_master arena -s 'onnx:model_v15,rollout$,sadist' tourney swiss 20
```

Player spec constraint suffixes (encoded in the ID, used for filtering):
- `|s5` or `|s5,7` - restrict to specific board size(s)
- `|hh` - hidden-hand mode only; `|hv` - visible-hand only

Models are looked up in `./models` by default. Override with `--models-dir`.

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


<br>

<sup>
	This repository follows <a href="https://github.com/valeratrades/.github/tree/master/best_practices">my best practices</a> and <a href="https://github.com/tigerbeetle/tigerbeetle/blob/main/docs/TIGER_STYLE.md">Tiger Style</a> (except "proper capitalization for acronyms": (VsrState, not VSRState) and formatting). For project's architecture, see <a href="./docs/ARCHITECTURE.md">ARCHITECTURE.md</a>.
</sup>

#### License

<sup>
	Licensed under <a href="LICENSE">Blue Oak 1.0.0</a>
</sup>

<br>

<sub>
	Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be licensed as above, without any additional terms or conditions.
</sub>

