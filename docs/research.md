# AI Research: Robot Master

Two open tracks: **Stockfish-style improvements** on the existing transformer, and **hidden-hands variant** which is playable but untrained and unoptimized.

---

## State Representation

~33 input planes on an NxN grid (already implemented in `encoding.rs`):

| Planes | Content |
|--------|---------|
| 6 | Card value presence (one binary plane per value 0-5) |
| 1 | Empty cells |
| 1 | Playable cells (adjacent to occupied) |
| 12 | Current player's hand (2 planes per card value, encoding count) |
| 12 | Opponent's hand (same encoding) |
| 1 | Current player indicator |

---

## Transformer Model

Encoder-only transformer, each board cell = 1 token. Already implemented in `py_src/model_transformer.py`.

| Property | Value |
|----------|-------|
| Tokens | N² (25 at 5x5, 121 at 11x11) |
| Embedding dim | 256 |
| Attention heads | 8 |
| Encoder layers | 4-6 |
| Policy head | per-token logits × card values = N² × 6 |
| Value head | CLS token → MLP → scalar [-1, 1] |

Key design: 2D geometric attention bias (Chessformer) encodes row/col structure relevant for line-based scoring.

| Paper | Year | Key Insight |
|-------|------|-------------|
| [AlphaViT](https://arxiv.org/abs/2408.13871) | 2024 | ViT replaces ResNet in AlphaZero; single model handles variable board sizes |
| [ResTNet](https://arxiv.org/html/2410.05347v2) | 2025 | Attention learns game concepts (alive groups, territory) not captured by conv |
| [Chessformer](https://arxiv.org/html/2409.12272v2) | 2024 | 6M-param transformer matches 270M-param CNN; geometric attention bias |
| [Gumbel MuZero](https://openreview.net/forum?id=bERaNdoegnO) | 2022 | Planning with 2-16 sims instead of 800 |

---

## Stockfish-Style Improvements

### 1. Alpha-Beta + Classical Pruning

Switch from Gumbel MCTS to iterative-deepening alpha-beta, using the transformer as an eval function. Relevant pruning at ~25 avg branching factor (5x5):

- **LMR (Late Move Reductions)**: moves sorted later get reduced-depth search. "Interesting" moves = plays improving your minimum line or blocking opponent's minimum.
- **Null move pruning**: skip a turn, reduced-depth search, prune if still above beta.
- **Aspiration windows**: start narrow around previous iteration's score.
- **Singular extensions**: one clearly-best move at reduced depth → extend it.
- **ProbCut**: shallow search with loose bound prunes clearly dominated lines.

NNUE synergy holds: every pruning decision relies on eval quality. The transformer's accurate leaf evaluations make all pruning techniques safer.

Trade-off: Gumbel MCTS is GPU-batchable and gives policy improvement guarantees. Alpha-beta is better when eval quality is high and you want maximum tactical depth on CPU. At depth 8-10 with ~25 branching factor this is very practical.

### 2. WDL Heads + Score Calibration

Output `(win, draw, loss)` probabilities instead of scalar `[-1, 1]`. Calibrate so +100 eval units = 50% win probability.

**Why**: Robot Master has draws. Explicit `draw` probability lets the network model "this position is drawn with high confidence" vs "this position is unclear" — collapsing to a signed scalar loses this. Also enables better-calibrated aspiration windows.

This is what Stockfish and LC0 both do. Low implementation cost, high signal quality gain.

### 3. Dual Network / Lazy Eval

Train a tiny fast evaluator alongside the transformer. Use it first:
- If the fast eval score is beyond a threshold (one player's current minimum line score is already above the opponent's best achievable total), skip the transformer.
- Otherwise run the transformer.

The "clearly decided" signal is O(N) to compute directly from game state. No NN needed for terminal-ish positions. This is SF's `use_smallnet()` / lazy evaluation. Direct analogue.

### 4. INT8 Quantization

ONNX Runtime supports post-train INT8 quantization via `quantize_dynamic` / `quantize_static`. Try this first before any custom work.

**For WASM deployment**: small transformer with int8 quantization via ONNX Runtime Web. Transformers at 5x5 (25 tokens, 256 dim) are tiny — int8 gets real-time inference in the browser.

---

## Hidden-Hands Variant

Already supported at the engine level (`--hide` flag, `GameConfig::hide`), self-play training supports it (`--hide` in the train CLI). What's missing: a model actually trained for it, and any search/network optimisation that accounts for imperfect information.

### The Problem

In visible-hand mode the full state is known → perfect information → standard MCTS/AlphaZero applies cleanly. In hidden-hands mode each player sees only their own hand and the board → imperfect information → the opponent's hand is a hidden variable. Naive MCTS is not principled here.

### Approach 1: ISMCTS (simplest baseline)

**Information Set MCTS** (Cowling, Powley, Whitehouse, 2012): at each node, sample a determinization (a concrete opponent hand consistent with observations), run MCTS on that perfect-information instance, aggregate statistics across samples. Cheap to implement on top of existing MCTS infrastructure.

Weakness: determinization can be misleading (optimal play under one sampled hand can be terrible under another). Works well in practice for moderate hidden-variable games, but is not theoretically sound.

### Approach 2: Belief-Augmented Network

Augment the transformer input: instead of the 12 "opponent's hand" planes (which are now hidden), substitute a **belief distribution** — a probability vector over possible opponent hands given the cards seen so far. The network learns to play well against uncertainty rather than against a known opponent.

Training: during self-play with `--hide`, the data generator only encodes what the current player can observe. The training objective is unchanged (minimize policy/value loss), but the network must learn to reason over belief states.

This is the standard approach in research (see ReBeL, Student of Games).

### Approach 3: CFR-Based Search

For a more principled treatment: **Counterfactual Regret Minimization** applied to the game tree. Relevant for finding Nash-approximate strategies rather than just strong heuristic play.

Key papers:

| Paper | Authors | Key Insight |
|-------|---------|-------------|
| [Student of Games](https://www.science.org/doi/10.1126/sciadv.adg3256) | Schmid et al. (2023) | Unified algorithm combining AlphaZero (perfect info) with PIMC+CFR (imperfect info); single framework handles both |
| [ReBeL](https://arxiv.org/abs/2007.13544) | Brown et al. (2020) | Recursive belief-space RL+search; subgame solving in public belief states; beats poker pros |
| ISMCTS | Cowling et al. (2012) | Determinization-based MCTS for imperfect info; practical baseline |

Martin Schmid's work (DeepMind, now elsewhere) is the most directly relevant: Student of Games is specifically designed for the class of games where you might want to train a single agent that handles both perfect and imperfect information variants. That's exactly our situation.

### What to Try First

1. Train a hidden-hands transformer with `--hide` using the existing self-play infrastructure (no algorithmic changes, just a model checkpoint trained on hidden-hand games). Establishes a baseline.
2. Measure the Elo gap between visible-hand and hidden-hand trained models on hidden-hand games.
3. Add belief-state encoding to the transformer input (replace hidden opponent hand planes with a belief distribution derived from deck knowledge).
4. If principled Nash-convergent play matters: implement ISMCTS first (cheap), then consider ReBeL-style subgame solving as a research project.

---

## Reference Implementations

| Project | Language | What to learn |
|---------|----------|---------------|
| [kZero](https://github.com/KarelPeeters/kZero) | Rust+Python | Exactly our architecture: Rust self-play, Python training, ONNX bridge |
| [MiniZero](https://arxiv.org/abs/2310.11305) | C++/Python | Gumbel AlphaZero/MuZero reference |
| [OpenSpiel](https://github.com/google-deepmind/open_spiel) | C++/Python | CFR variants, ISMCTS, imperfect info baselines; Schmid et al. code often lands here |
