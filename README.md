# robot_master_site <img width="25%" src="https://www.jeuxdenim.be/images/jeux/RobotMaster_large01.jpg" alt="Robot Master">
![Minimum Supported Rust Version](https://img.shields.io/badge/nightly-1.92+-ab6000.svg)
[<img alt="crates.io" src="https://img.shields.io/crates/v/robot_master_site.svg?color=fc8d62&logo=rust" height="20" style=flat-square>](https://crates.io/crates/robot_master_site)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs&style=flat-square" height="20">](https://docs.rs/robot_master_site)
![Lines Of Code](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/robot_master_site-loc.json)
<br>
[<img alt="ci errors" src="https://img.shields.io/github/actions/workflow/status/valeratrades/robot_master_site/errors.yml?branch=master&style=for-the-badge&style=flat-square&label=errors&labelColor=420d09" height="20">](https://github.com/valeratrades/robot_master_site/actions?query=branch%3Amaster) <!--NB: Won't find it if repo is private-->
[<img alt="ci warnings" src="https://img.shields.io/github/actions/workflow/status/valeratrades/robot_master_site/warnings.yml?branch=master&style=for-the-badge&style=flat-square&label=warnings&labelColor=d16002" height="20">](https://github.com/valeratrades/robot_master_site/actions?query=branch%3Amaster) <!--NB: Won't find it if repo is private-->

multi-player implementation of robot_master // in rust, because of course it is


### Reqs and `py_src/`
provisioned pdf with requirements: ./Sujet-RobotMaster-version-04-02.pdf

rough arch outline, functionality of each function, tests, desired behavior, - can be found in this pdf file

## Rules
1v1 on a 5x5 grid. Cards are numbered 0–5, with 6 copies each (36 total). Each player gets 12; a 25th card is placed at the center of the board.

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
<h3>Installation</h3>
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

#### Without Nix
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

# install python dependencies
pip install typeguard icecream
# (dev: pip install pytest ruff inline-snapshot)

# build python bindings (required for `python -m py_src` to work)
maturin develop --features python
```

</details>
<!-- markdownlint-restore -->

## Usage
### Usage

The main binary is `robot_master`. It takes two players (`-a`, `-b`), an optional board size (`-s`), and a subcommand for the interface.

Players: `manual` (`m`), `random` (`r`), `greedy` (`g`), `sadist` (`s`). Unrecognized names prompt registration as a named manual player (with Elo tracking), or fall back to `fzf` selection.

Board sizes: `5` (default), `7`, `9`, `11`.

#### TUI
```sh
robot_master tui                            # you vs random AI, 5x5
robot_master tui -a greedy -b sadist -s 7   # watch two AIs fight on 7x7
robot_master tui -a Alice -b Bob            # two named humans, Elo tracked
```
In manual mode, the TUI prompts for card, row, column each turn. Invalid moves get a warning and re-prompt.

#### GUI
```sh
robot_master gui
robot_master gui -a manual -b greedy
```
Bevy app with a main menu where you can pick players and board size from dropdowns before starting. Elo ratings are shown next to player names.

#### Python
For running the project as pure Python (e.g. for grading), the Rust binary must be compiled first (`cargo b -p robot_master`). The Python modules in `py_src/` shell out to it.

```sh
python -m py_src guided -m   # partie guidée, manual (both players)
python -m py_src guided -r   # partie guidée, random (both players)
python -m py_src naive -g    # IA mode, greedy vs greedy
python -m py_src naive -a    # IA mode, sadist vs sadist
```

#### Elo
Player ratings persist across games in `$XDG_DATA_HOME/robot_master/ratings.json`. Every named player (manual or AI) accumulates an Elo score. End-of-game output shows rating changes.



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

