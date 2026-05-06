# robot_master
![Minimum Supported Rust Version](https://img.shields.io/badge/nightly-1.92+-ab6000.svg)
[<img alt="crates.io" src="https://img.shields.io/crates/v/robot_master.svg?color=fc8d62&logo=rust" height="20" style=flat-square>](https://crates.io/crates/robot_master)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs&style=flat-square" height="20">](https://docs.rs/robot_master)
![Lines Of Code](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/robot_master-loc.json)
<br>
[<img alt="ci errors" src="https://img.shields.io/github/actions/workflow/status/valeratrades/robot_master/errors.yml?branch=master&style=for-the-badge&style=flat-square&label=errors&labelColor=420d09" height="20">](https://github.com/valeratrades/robot_master/actions?query=branch%3Amaster) <!--NB: Won't find it if repo is private-->
[<img alt="ci warnings" src="https://img.shields.io/github/actions/workflow/status/valeratrades/robot_master/warnings.yml?branch=master&style=for-the-badge&style=flat-square&label=warnings&labelColor=d16002" height="20">](https://github.com/valeratrades/robot_master/actions?query=branch%3Amaster) <!--NB: Won't find it if repo is private-->

A board game built as a tractable environment for studying modern game AI — transformers, Gumbel AlphaZero self-play, and imperfect information search. Small enough to train to competent play on a single GPU overnight; complex enough to be non-trivial (non-linear scoring, asymmetric objectives, state space ~10¹⁵ at 5×5 scaling to ~10⁷⁰ at 11×11).

<div align="center">
<table width="68%" cellspacing="4" cellpadding="0" border="0">
  <tr>
    <td width="50%" rowspan="2" valign="top">
      <img width="1280" height="1586" alt="scrn-2026-05-06-17-01-28" src="https://github.com/user-attachments/assets/66ea6a35-31d0-4071-8417-58a4123ae3c3"/>
    </td>
    <td width="50%">
      <img width="1279" height="794" alt="scrn-game" src="https://github.com/user-attachments/assets/5480426d-dabe-4dea-8c1f-22285ba22588" />
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img width="1279" height="791" alt="scrn-result" src="https://github.com/user-attachments/assets/4f447ed0-f819-4ba6-b41d-71b6b0ebe80c" />
    </td>
  </tr>
</table>
</div>

### What's been built

- **Gumbel AlphaZero pipeline** — self-play in Rust, training in PyTorch, ONNX as the runtime contract. [Gumbel MuZero](https://openreview.net/forum?id=bERaNdoegnO) (Danihelka et al., ICLR 2022): works with 2–16 MCTS sims per move instead of the 400–800 vanilla AlphaZero needs.
- **Encoder-only transformer** — board cells as tokens, geometric attention bias ([Chessformer](https://arxiv.org/abs/2409.12272)), single model scales across board sizes 5×5 → 11×11 without retraining.
- **Arena** — Glicko-2 ratings, Swiss/round-robin/elimination tournaments. Trained ONNX models plug in as arena players against each other and built-in bots.
- **Bevy GUI + Leptos web app + TUI** — all const-generic over board size N ∈ {5, 7, 9, 11}.
- **Hidden-hands variant** — opponent's hand is hidden; engine and self-play support it, dedicated training and search not yet done.

### What's next

- **Alpha-beta search** with the transformer as eval — LMR, null-move pruning, aspiration windows. Stockfish-style depth where MCTS gives breadth.
- **WDL heads** — explicit win/draw/loss output instead of scalar value. Draws matter here and the scalar collapses them.
- **Imperfect information search** for the hidden-hands variant: ISMCTS baseline → belief-augmented transformer input → [ReBeL](https://arxiv.org/abs/2007.13544) / [Student of Games](https://www.science.org/doi/10.1126/sciadv.adg3256) (Schmid et al., 2023).

See [`docs/research.md`](docs/research.md) for details.

### Rules

1v1 on a 5×5 grid. Cards are numbered 0–5, with 6 copies each (36 total). Each player gets 12; a 25th card is placed at the center.

**Turns**: players alternate placing a card from their hand onto an empty cell adjacent (no diagonals) to an occupied one.

**Scoring** (per line/column, once the grid is full):
| copies of a card | points |
|---|---|
| 1 | face value |
| 2 | 10 × face value |
| 3+ | 100 flat |

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

Then build the Rust binary:
```sh
cargo b -p robot_master
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
- [`fzf`](https://github.com/junegunn/fzf) (optional, for player name selection in TUI)

##### Steps
```sh
# build the main binary
cargo b -p robot_master

# training dependencies (torch, onnx, tensorboard)
pip install torch numpy onnx onnxruntime tensorboard
```

</details>
<!-- markdownlint-restore -->

## Usage
### Usage

```sh
robot_master gui                          # main menu, pick players and board size
robot_master gui -a manual -b random     # skip straight to a game
```
To get a live eval bar, click `Settings -> Eval Mode`

Built-in players: `manual`, `random`, `greedy`, `sadist`, `rollout`. Board sizes: `5`, `7`, `9`, `11`. Append `|g<N>` to wrap any bot in Gumbel MCTS (`sadist|g200`). Named human players get Elo tracked automatically.

<!-- markdownlint-disable -->
<details>
<summary>
<h3>TUI / Arena</h3>
</summary>

```sh
# one-off match without touching saved ratings
robot_master arena --no-priors 'random,onnx:model_v15|g200' tourney swiss 20

# register a trained model, then run it in tournaments
robot_master arena players new 'onnx:model_v15|g200' --sizes 5
robot_master arena tourney swiss 10
```

`robot_master --help` covers the full player spec syntax, tourney modes, arena player management, and `--hide` (hidden-hand mode).

</details>
<!-- markdownlint-restore -->

## Training

One iteration of the training loop:
1. **Self-play** (Rust, parallel via rayon) - Gumbel AlphaZero games write `(state, policy, value)` samples to `$XDG_CACHE_HOME/robot_master_train/<generation>/training_data/`
2. **Train** (Python) - model fits on the replay buffer, saves checkpoint to `$XDG_CACHE_HOME/robot_master_train/<generation>/models/`. Optimizer state is carried forward across iterations (SGD momentum preserved).
3. **Export** (Python) - latest checkpoint → `models/model_vN.onnx` for the next self-play iteration

```sh
robot_master train transformer --iterations 100 --games 400 --sims 25
robot_master train cnn --supervise 'rollout|v50' #XXX: haven't gotten it to converge, - is either misimplemented or just much weaker than transformers in general
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

