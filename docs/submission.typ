#set page(margin: 1.5cm)
#set text(font: "New Computer Modern", size: 11pt)
#set heading(numbering: "1.")
#show link: underline

#align(center)[
  #text(size: 14pt, weight: "bold")[UE Projet informatique - L1 S2]
  #linebreak()
  #text(size: 16pt, weight: "bold")[Projet : RobotMaster]
  #v(0.5em)
  Valeriy Sakharov - Mi3-Bin2
  #linebreak()
  Année académique 2025/2026
]

#outline()

= GitLab repository

#link("https://gitlab.isima.fr/vasakharov/robot_master")

Relevant tags:

- `v0.2.0` - last pure-Python version. Full guided part + AI strategies, no Rust.
- `v0.5.0` - current state. Engine in Rust, PyO3 bindings all passing, full extensions.

= Work summary

Started with the Python implementation the assignment asked for. After `v0.2.0` (everything done per spec, all tests passing), rewrote the game engine in Rust for performance - the AlphaZero self-play pipeline generates millions of positions and Python doesn't scale to that. The Python code in `py_src/` is still there and still runs; PyO3 bindings expose the Rust engine so the distributed tests keep working without modification.

Full architecture is in #link("https://gitlab.isima.fr/vasakharov/robot_master/-/blob/master/docs/ARCHITECTURE.md")[`docs/ARCHITECTURE.md`]. This report covers what you can't read directly from the code.

= Python implementation (`py_src/`)

All required functions are implemented across `partie_guidee/` and `IA/` exactly per spec. Two implementation choices worth noting:

`distribution_cartes`: the deck is shuffled once with `random.shuffle`, then sliced - first card to the board center, then `cartes_distrib`-sized chunks to each player. One shuffle, no rejection sampling.

`complete_et_score`: recursive. Base case: line is full, call `score_ligne`, append to results. Otherwise: for each card value present in `dico_cartes_restantes`, place it in the next empty cell, decrement the counter, recurse, then restore. The dict is mutated in-place and restored after each branch - no deepcopy per level.

== Deviations from spec

`c_test.py` line 65: the original test used a list `["B", "r", {3: 1}]` for a player entry. Changed to a tuple `("B", "r", {3: 1})` - the first two fields are positional and immutable, a tuple is the right type.

`IA_test.py`: this file wasn't in the skeleton I had access to early on, so it doesn't exist or pass before `v0.5.0`. The assertions there are written against final score rather than exact board state, so tiebreaker rules don't affect them.

= Extensions

== Rust engine + PyO3 bindings

#link("https://gitlab.isima.fr/vasakharov/robot_master/-/tree/master/robot_master_core")[`robot_master_core`] is the game engine: board, cards, scoring, move generation. Const-generic on board size `N` (supports 5x5 through 11x11). `python.rs` is a thin PyO3 surface that keeps the required test signatures intact.

== GUI

Bevy app with a main menu - pick players and board size from dropdowns, Elo ratings shown next to names. Compiles to WebAssembly.

#link("https://gitlab.isima.fr/vasakharov/robot_master/-/blob/master/README.md#gui")[README - GUI section] for how to run it.

```sh
robot_master gui
```

== Tournament arena + Glicko-2

#link("https://gitlab.isima.fr/vasakharov/robot_master/-/tree/master/robot_master_arena")[`robot_master_arena`] runs tournaments between any registered players. Ratings persist in JSON via Glicko-2.

#link("https://gitlab.isima.fr/vasakharov/robot_master/-/blob/master/README.md#arena")[README - Arena section] for the full interface.

```sh
robot_master arena tourney swiss 10
robot_master arena players list
```

Current leaderboard:

#raw(lang: "text",
"  rollout|v800: 1712 (RD 104)
  rollout|v200: 1596 (RD  86)
  rollout|v50:  1512 (RD 100)
  gfn:          1446 (RD  81)
  rollout|g50:  1217 (RD  86)
  gfs:          1123 (RD  81)
  rollout|g200: 1117 (RD  80)
  rollout|g800: 1105 (RD  80)
  rollout:      1080 (RD  84)
  sadist:       1065 (RD  85)
  random:       1051 (RD  86)
  onnx:model_v114:      976? (RD 133)
  onnx:model_v114|g200:  891 (RD  93)
  ")

`rollout|vN` / `rollout|gN` = MCTS with N simulations (vanilla UCT vs Gumbel). `gfn`/`gfs` = frozen snapshots of the Gumbel search tuning experiments.

== AlphaZero pipeline

#link("https://gitlab.isima.fr/vasakharov/robot_master/-/tree/master/robot_master_train")[`robot_master_train`] handles self-play. #link("https://gitlab.isima.fr/vasakharov/robot_master/-/tree/master/training")[`training/`] handles the PyTorch side. The interface between them is the filesystem: `.bin` sample files out of Rust, `.onnx` models back in.

Search algorithm is Gumbel AlphaZero (Sequential Halving with Gumbel noise) rather than vanilla MCTS - converges with 8-16 simulations per move instead of 400+, which matters when you need to generate enough data. Detailed rationale and architecture in #link("https://gitlab.isima.fr/vasakharov/robot_master/-/blob/master/docs/research.md")[`docs/research.md`].

Latest model (SE-ResNet 5 blocks / 64 filters, ~407K params) is still only slightly better than `random` lol - found a critical bug literally an hour ago. Pipeline is end-to-end functional, just ran out of time before submission.

= Installation

```sh
nix build
```

Yes, that easy. Install the package manager, and you're all set. For more read #link("https://gitlab.isima.fr/vasakharov/robot_master/-/blob/master/README.md#installation")[README - Installation], not gonna re-explain it.

= Bibliography

- DeepMind, "Mastering the Game of Go without Human Knowledge" (AlphaGo Zero), _Nature_ 2017
- DeepMind, "A General Reinforcement Learning Algorithm..." (AlphaZero), _Science_ 2018
- Danihelka et al., "Policy Improvement by Planning with Gumbel", ICLR 2022 - #link("https://openreview.net/forum?id=bERaNdoegnO")
- Ye et al., "MiniZero: Comparative Analysis of AlphaZero and MuZero", AAAI 2024 - #link("https://arxiv.org/abs/2310.11305")
- KarelPeeters/kZero (Rust self-play + Python training reference) - #link("https://github.com/KarelPeeters/kZero")
