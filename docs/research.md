# AI Research: Robot Master

Robot Master is a perfect-information, zero-sum, combinatorial game on an NxN board (5-11). 24 moves per game at 5x5, average branching factor ~25. State space ~10^15 at 5x5, growing dramatically with board size.

Two parallel tracks: **Track A** is the proven approach to crush this game fast. **Track B** is a Transformer-based approach optimized for learning transferable ML skills (scales to larger boards, generalizes to other domains like finance).

---

## State Representation (Input Planes)

Like AlphaGo's 19x19 input planes, but NxN:

| Planes | Content |
|--------|---------|
| 6 | Card value presence (one binary plane per value 0-5) |
| 1 | Empty cells |
| 1 | Playable cells (adjacent to occupied) |
| 12 | Current player's hand (2 planes per card value, encoding count) |
| 12 | Opponent's hand (same encoding) |
| 1 | Current player indicator |

~33 input planes on an NxN grid.

### Why This Works for Robot Master

| Property | Go | Robot Master (5x5) | Robot Master (11x11) |
|----------|-----|-------------|-------------|
| Board | 19x19 | 5x5 | 11x11 |
| Branching factor | ~250 | ~25 avg | ~100+ avg |
| Game length | ~200 | 24 | 120 |
| State space | ~10^170 | ~10^15 | ~10^70+ |

The scoring function is highly nonlinear (1 copy = face value, 2 copies = 10x, 3+ = 100 flat). Random MCTS rollouts won't discover these interactions — that's exactly why a learned value function guiding search dominates here.

The min-across-lines objective creates a "weakest link" dynamic: shore up your worst line while attacking the opponent's worst. The neural net needs to learn this balance, similar to territorial balance in Go.

---

## Track A: AlphaZero (Crush the Game)

The standard, proven approach. Small ResNet + MCTS + self-play. Goal: strongest possible play, fastest convergence.

### Network Architecture

Small ResNet (5 residual blocks, 64 filters — board is only 5x5 to start):

- **Policy head**: probability over all (card, position) pairs — 6 x N² logits
- **Value head**: scalar in [-1, 1], win probability

### Algorithm: Gumbel AlphaZero

Use Gumbel AlphaZero over vanilla AlphaZero. Key advantage: works reliably with as few as 8-16 MCTS simulations per move during training (vanilla needs 400-800). Massively faster self-play.

Reference: [Policy Improvement by Planning with Gumbel](https://openreview.net/forum?id=bERaNdoegnO)

### Training Cycle

```
┌─────────────────────────────────────────────┐
│              ITERATION CYCLE                 │
│                                              │
│  1. Self-Play Generation (Rust)              │
│     ├─ Latest network plays itself           │
│     ├─ MCTS, 16-64 sims/move                │
│     ├─ Temperature τ=1 early, τ→0 late       │
│     └─ Write (state, π, z) to disk           │
│         π = MCTS visit counts (policy target)│
│         z = game outcome (+1/-1)             │
│                                              │
│  2. Training (Python/PyTorch)                │
│     ├─ Read game data from disk              │
│     ├─ Sample minibatches from replay buffer │
│     ├─ Loss = L_value(v, z) + L_policy(p, π) │
│     │        + c·‖θ‖²                        │
│     ├─ SGD with momentum, ~1000 steps        │
│     └─ Export model.onnx                     │
│                                              │
│  3. Repeat (~50-100 iterations)              │
└─────────────────────────────────────────────┘
```

Note: AlphaGo Zero had a separate evaluation step (new net vs current best, 400 games, promote only if win rate > 55%). AlphaZero dropped this entirely — the latest checkpoint is always used for the next self-play iteration. This was found to make no difference in practice and halves iteration overhead. We follow AlphaZero.

Expected training: 4-12 hours on a single modern GPU (RTX 3080/4080) for 5x5.

### Ideas to Steal

**From Stockfish NNUE:**
1. **Incremental evaluation** — board changes by one card per move, update accumulator instead of re-evaluating
2. **Quantized inference** — int8/int16 for fast WASM deployment in the Bevy game

**From AlphaGo/AlphaZero:**
3. **Dirichlet noise at root** — exploration during self-play, prevents policy collapse
4. **Virtual loss in parallel MCTS** — prevents threads from exploring same path
5. **Symmetry augmentation** — 90° rotation + player swap is equivalent, multiplies training data

---

## Post-Training NNUE Optimizations (Phase 5 detail)

Once the AlphaZero eval head is trained and working, the following optimizations replicate the Stockfish NNUE stack. Priority-ordered.

### 1. Flat Feature Representation + Incremental Accumulator (highest ROI)

**The core NNUE insight:** The first linear layer is a sum of active feature columns. When a move changes O(1) features, you add/subtract those columns instead of recomputing the full matrix-vector multiply. 10-15x faster than full recomputation.

**What changes:**
- Ditch the CNN. Replace with a flat feature set: `(cell_square, card_value, occupancy)` — same information, different encoding. ~150 active features out of a ~2000-dim feature space (<0.1% sparse).
- First layer is a `[features × hidden_dim]` weight matrix. Accumulator = sum of active feature columns + bias.
- Maintain an accumulator stack as the search tree descends. On make/unmake move: add columns for new features, subtract columns for removed features. Full recompute only on "dirty" ancestors.
- Separate accumulators for each player's perspective (white/black equivalent = rows player/cols player). Already done via board transpose in `encoding.rs` — same principle, now needs to live in the accumulator.

**Prerequisite:** This requires redesigning the network architecture away from CNN. The current SE-ResNet cannot be incrementally updated because conv layers are spatially entangled. Same game information, different representation.

### 2. Game-Phase Bucketing (HalfKP analogue)

In chess, features are indexed as `(king_square, piece_square, piece_type)` — all piece values are relative to your king position. There's no king here, but the natural analogues:

- **Turn-number buckets:** Divide the game into 4-8 phase buckets by `turn / total_turns`. The network weights can differ per bucket — early game (spread out) vs late game (shore up your worst line) need different evaluation logic.
- **Line-state bucketing:** Features indexed by `(scoring_line_id, cell_position, card_value)` so the network explicitly represents "this card matters because it's on line X which is my current minimum." Analogous to king-relative piece values.
- **Threat features (SFNNv10 analogue):** Add explicit `(line_id, current_minimum_card, cards_remaining_in_line)` as input features — the state of each scoring line directly in the input, not inferred through conv layers.

### 3. INT8/INT16 Quantization

**Feature transformer:** Store weights as int16 (scaled by 127). Accumulator in int16. With ~150 active features × max weight ~127, accumulator stays in int16 range.

**Dense hidden layers:** After ClippedReLU (clamp to [0, 127]), all subsequent layers use int8 × int8 → int32 SIMD. AVX2 processes 32 int8 multiplications per instruction. This is 4-8x faster than float32 on CPU.

**In practice:** ONNX Runtime already supports post-train INT8 quantization via the `quantize_dynamic` / `quantize_static` APIs — try this first before rolling custom SIMD. If you go full custom NNUE in Rust, use `std::arch` AVX2 intrinsics.

### 4. Dual Network (Big + Small)

Train two networks: a full-quality network and a tiny MLP (or even hand-crafted classical eval). Use a cheap `simple_eval()` first:
- If `simple_eval()` is beyond a threshold (clearly decided — one player has locked in a dominant line), skip the full network.
- Otherwise, run the full network.

The "clearly decided" signal for Robot Master is easy to compute in O(N): if a player's current minimum line score is already above the opponent's best achievable total, the game is won. No NN needed.

This is SF's `use_smallnet()` / lazy evaluation strategy. Direct analogue.

### 5. Alpha-Beta + Classical Pruning (Architecture Switch)

The biggest architectural question. Switching from Gumbel MCTS to iterative-deepening alpha-beta gives:

- **LMR (Late Move Reductions):** Moves sorted later in the ordered list get searched at reduced depth. Only "interesting" moves (plays that improve your minimum line, block opponent's minimum) get full depth.
- **Null move pruning:** Pass a turn, do a reduced-depth search. If still above beta, prune. Applicable since tempo has real value.
- **Aspiration windows:** Start with a narrow window around the previous iteration's score. Converges fast when your evaluator is stable.
- **Singular extensions:** If one move is clearly best at reduced depth, extend it — searches the critical line deeper.
- **ProbCut:** Shallow search with a loose bound prunes clearly dominated lines.

**Trade-off:** Gumbel MCTS already gives policy improvement guarantees and is GPU-batchable. Alpha-beta is better when eval quality is high and you want maximum tactical depth on a CPU. With Robot Master's branching factor (~25 avg at 5x5), alpha-beta at depth 8-10 is very practical.

NNUE synergy: every pruning decision (null move, LMR, ProbCut) relies on the accuracy of the shallow eval. NNUE's accurate leaf evaluations make all of these pruning techniques safer and more aggressive.

### 6. WDL Heads + Score Calibration

Instead of a single value in `[-1, 1]`, output `(win, draw, loss)` probabilities separately. Calibrate so +100 eval units = 50% win probability in self-play games. This is what Stockfish and LC0 both do now.

**Why:** Better-calibrated uncertainty estimates. Aspiration windows can be set in expected-value terms. The draw probability matters — Robot Master has draws, and the network should explicitly model them rather than collapsing to a signed scalar.

### Summary Table

| Optimization | Effort | Speedup/Benefit | Prerequisite |
|---|---|---|---|
| Flat features + incremental accumulator | High | 10-15x eval speed | Ditch CNN |
| Game-phase bucketing | Medium | Better eval quality | Flat features |
| INT8 quantization | Low (ONNX API) | 4-8x CPU throughput | None |
| Dual network / lazy eval | Medium | 2-3x avg speed | Working NN |
| Alpha-beta + pruning | High | Exponential depth gain | Good eval |
| WDL heads | Low | Better calibration | None |

---

## Track B: Transformers (Learn ML, Scale Up)

Goal: learn modern ML practices that transfer beyond board games. Transformers are the architecture that matters — vision, language, time series, finance all converge on attention.

### Why Transformers Here

- **Variable board sizes**: a single model handles 5x5 through 11x11 (ResNet needs retraining per size)
- **Global attention**: at 9x9+, CNN receptive fields struggle with whole-board patterns. Transformers see everything
- **Transferable skills**: attention mechanisms, positional encodings, training dynamics — all transfer to sequence modeling (stocks, time series)
- **Research frontier**: AlphaViT (2024), ResTNet (IJCAI 2025), Chessformer (2024) show transformers matching or beating CNNs for game playing

### Architecture

Encoder-only transformer, each board cell = 1 token:

- **Input**: N² tokens, each embedding card value + position + hand context
- **Positional encoding**: 2D geometric attention bias (row/col structure matters for scoring)
- **Body**: 4-6 encoder layers, 256 embedding dim, 8 attention heads
- **Policy head**: per-token logits × card values = N² × 6 output
- **Value head**: CLS token or mean-pool → MLP → scalar [-1, 1]

At 5x5: 25 tokens — attention is trivially cheap.
At 11x11: 121 tokens — still tiny by transformer standards (GPT handles 128K).

### Key Papers to Study

| Paper | Year | Key Insight |
|-------|------|-------------|
| [AlphaViT](https://arxiv.org/abs/2408.13871) | 2024 | ViT replaces ResNet in AlphaZero; handles variable board sizes with single model |
| [ResTNet](https://arxiv.org/html/2410.05347v2) | 2025 | CNN+Transformer hybrid; attention learns game concepts (alive stones, territory) |
| [Chessformer](https://arxiv.org/html/2409.12272v2) | 2024 | 6M-param transformer matches 270M-param CNN for value estimation |
| [Gumbel MuZero](https://openreview.net/forum?id=bERaNdoegnO) | 2022 | Planning with 2-16 sims instead of 800; works with any network arch |

### What to Learn (in order)

1. **Attention mechanism from scratch** — Karpathy's "Let's build GPT" gets you 80% of the way
2. **Vision Transformer (ViT)** — how to tokenize a 2D grid, positional embeddings for spatial data
3. **AlphaZero training loop** — MCTS + self-play + policy/value loss (same for both tracks)
4. **Geometric attention bias** — Chessformer's key insight for board games
5. **Training dynamics** — learning rate scheduling, gradient clipping, batch normalization vs layer normalization (transformers use LayerNorm)

---

## System Architecture

Both tracks share the same Rust ↔ Python split. No FFI, no bindings — filesystem is the interface.

```
robot_master_train/            (Rust crate)
├── src/
│   ├── mcts.rs                MCTS tree search + Evaluator trait
│   ├── selfplay.rs            game generation loop
│   ├── nn.rs                  ONNX Runtime inference wrapper
│   ├── encoding.rs            GameState → tensor, training data serialization
│   └── eval.rs                network-vs-network evaluation
├── src/bin/
│   ├── selfplay.rs            CLI: generate N games, write to disk
│   └── evaluate.rs            CLI: new model vs current best
└── Cargo.toml

training/                      (Python, NOT a Rust crate)
├── model_resnet.py            Track A: small ResNet
├── model_transformer.py       Track B: encoder-only transformer
├── train.py                   training loop (shared between tracks)
├── export_onnx.py             PyTorch → ONNX conversion
└── requirements.txt
```

**Data flow:**
```
Rust selfplay  ──writes──>  training_data/*.bin
Python train   ──reads───>  training_data/*.bin
Python train   ──writes──>  models/model_v{N}.onnx
Rust selfplay  ──reads───>  models/model_v{N}.onnx  (via ort crate)
Rust evaluate  ──reads───>  models/model_v{N}.onnx
```

**Rust-side inference**: `ort` crate (ONNX Runtime bindings). Supports CUDA/TensorRT for GPU inference during self-play. No libtorch dependency.

**Why not all-Rust training?** PyTorch's autograd, optimizers, LR schedulers, and debugging tools (tensorboard, wandb) are battle-tested. Reimplementing in Rust (via tch-rs/burn/candle) is pain for zero gain during training. Rust handles the hot path: self-play + inference.

---

## Roadmap

### Phase 1 — Fast Game Engine ✅
- Board state, move generation, scoring — done
- `robot_master_core` + `robot_master_arena` crates

### Phase 2 — MCTS Foundation
- Pure MCTS with greedy rollouts as baseline (no neural net)
- Implement in `robot_master_train` crate
- Will already beat Greedy and Sadist
- Establishes search framework and benchmarking

### Phase 3 — Track A: AlphaZero

#### Done ✅
- `training/model_resnet.py` — SE-ResNet (5 blocks, 64 filters, ~407K params), dual heads (policy + value), `encode_state`
- `training/export_onnx.py` — ONNX export with roundtrip validation
- `training/train.py` — training loop (SGD + cosine LR, TensorBoard logging, checkpointing)
- `robot_master_train/src/encoding.rs` — `encode_planes` (GameState → 33 CHW planes), `encode_sample` (→ binary)
- `robot_master_train/src/selfplay.rs` — `play_game` (MCTS self-play, records visit-count policy targets + retroactive value)
- `robot_master_train/src/bin/selfplay.rs` — CLI binary, rayon-parallel, writes `.bin` files

#### Phase 3a — Data quality (do before real training) ⬅ NEXT
1. **Temperature sampling in `play_game`** — currently picks most-visited move deterministically.
   AlphaZero uses τ=1 (sample ∝ visit counts) for first ~10 moves, τ→0 after.
   Adds diversity, prevents game collapse to a single line.
2. **Dirichlet noise at root** — add `α·Dir(0.3) + (1-α)·prior` at root during self-play.
   Prevents MCTS from always exploring the same moves. `α≈0.3`, noise weight `ε≈0.25`.

#### Phase 3b — NN inference in Rust
3. **`NnEval`** — implement `Evaluator<N>` backed by ONNX Runtime (`ort` crate).
   Until this exists, selfplay uses `RolloutEval` (random rollouts), which produces garbage policy targets.

#### Phase 3c — Full training cycle ✅
4. ~~**`bin/evaluate.rs`**~~ — dropped, following AlphaZero (see note above). `bin/evaluate.rs` kept as a diagnostic tool for arena comparisons but not part of the training loop.
5. **Replay buffer management** — `train.py` currently reads all `.bin` files. Cap to last K iterations to prevent forgetting (K≈20 typically).
6. **Iteration script** — `scripts/train_cnn.rs`: loops selfplay → train → export, always promotes latest checkpoint.

#### Long-term notes
- Gumbel AlphaZero replaces standard MCTS for training — works with 8-16 sims instead of 400. Implement after baseline works.
- Target: demolish all heuristic bots on 5x5

[^1] Gumbel takes knowledge of improvement values in this cycle, then explores the lines with the best ones repeatedly: rolls out ones it has, gets scores, cuts out bottom half, repeats. Basically, select the most promising point, then purposefully penetrate it; instead of sampling around the boundary.
I think this works cause we're more likely to discover good *lines* this way. But we have too much variance on next rollout on the first move we make in this direction. So here we just pre-compile commitment (think fuel in matklad's lexer).

### Phase 4 — Track B: Transformer
- Encoder-only transformer in PyTorch (`training/model_transformer.py`)
- Same MCTS + self-play infrastructure (swap the model, everything else identical)
- Train on 5x5 first, then scale to 7x7, 9x9, 11x11 without retraining from scratch
- Compare Elo curves: ResNet vs Transformer at each board size

### Phase 5 — Deployment
- Distill best model into efficient architecture for real-time play
- NNUE-style quantized eval for WASM (Bevy game)
- Or: small transformer with int8 quantization via ONNX Runtime Web

### Phase 6 — Hidden Hands Variant
- Extend engine to support imperfect information
- ISMCTS or belief-augmented search
- Compare open vs hidden Elo curves

---

## Reference Implementations

| Project | Language | What to learn from it |
|---------|----------|----------------------|
| [kZero](https://github.com/KarelPeeters/kZero) | Rust+Python | Exactly our architecture: Rust self-play, Python training, ONNX bridge |
| [alpha-zero-general](https://github.com/suragnair/alpha-zero-general) | Python | Simple reference for understanding the training loop |
| [MiniZero](https://arxiv.org/abs/2310.11305) | C++/Python | Gumbel AlphaZero/MuZero reference implementation |
| [michaelnny/alpha_zero](https://github.com/michaelnny/alpha_zero) | Python | Clean PyTorch AlphaZero for Go/Gomoku |
