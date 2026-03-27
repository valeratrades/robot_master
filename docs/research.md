# AI Research: Robot Master

## Recommended Approach: AlphaZero-style + NNUE Ideas

Robot Master is a perfect-information, zero-sum, combinatorial game on a 5x5 board. 24 moves per game, average branching factor ~25. State space ~10^15 — large enough to be intractable for brute-force, small enough that neural-guided search should converge fast.

### State Representation (Input Planes)

Like AlphaGo's 19x19 input planes, but 5x5:

| Planes | Content |
|--------|---------|
| 6 | Card value presence (one binary plane per value 0-5) |
| 1 | Empty cells |
| 1 | Playable cells (adjacent to occupied) |
| 12 | Current player's hand (2 planes per card value, encoding count) |
| 12 | Opponent's hand (same encoding) |
| 1 | Current player indicator |

~33 input planes on a 5x5 grid. Tiny compared to Go.

### Network Architecture

Small ResNet (5-10 residual blocks — board is only 5x5, no need for depth):

- **Policy head**: probability over all (card, position) pairs — 6 x 25 = 150 logits
- **Value head**: scalar in [-1, 1], win probability

### Training: The Iteration Cycle

```
┌─────────────────────────────────────────────┐
│              ITERATION CYCLE                 │
│                                              │
│  1. Self-Play Generation                     │
│     ├─ Current best network plays itself     │
│     ├─ MCTS with ~200-800 simulations/move   │
│     ├─ Temperature τ=1 early, τ→0 late       │
│     └─ Store (state, π, z) tuples            │
│         π = MCTS visit counts (policy target)│
│         z = game outcome (+1/-1)             │
│                                              │
│  2. Training                                 │
│     ├─ Sample minibatches from replay buffer │
│     ├─ Loss = L_value(v, z) + L_policy(p, π) │
│     │        + c·‖θ‖²                        │
│     └─ SGD with momentum, ~1000 steps        │
│                                              │
│  3. Evaluation                               │
│     ├─ New net vs current best: 400 games    │
│     ├─ If win rate > 55%: promote            │
│     └─ Else: discard, continue self-play     │
│                                              │
│  4. Repeat                                   │
└─────────────────────────────────────────────┘
```

Estimated convergence: ~50-100 iterations on a single GPU in hours.

### Why This Works for Robot Master

| Property | Go | Robot Master |
|----------|-----|-------------|
| Board | 19x19 | 5x5 |
| Branching factor | ~250 | ~25 avg |
| Game length | ~200 | 24 |
| State space | ~10^170 | ~10^15 |
| MCTS sims needed | 1600+ | 200-400 likely enough |

The scoring function is highly nonlinear (1 copy = face value, 2 copies = 10x, 3+ = 100 flat). Random MCTS rollouts won't discover these interactions — that's exactly why a learned value function guiding search dominates here.

The min-across-lines objective creates a "weakest link" dynamic: shore up your worst line while attacking the opponent's worst. The neural net needs to learn this balance, similar to territorial balance in Go.

---

## Ideas to Steal

### From Stockfish NNUE

1. **Incremental evaluation** — the board changes by one card per move. Don't re-evaluate the whole board; update an accumulator. For MCTS rollouts this means faster node evaluation.

2. **Feature factorization** — Stockfish uses HalfKP (king_square, piece_square). Here: factor as (card_value, position, line_context) — encoding not just what's placed where but how it interacts with line scoring.

3. **Quantized inference** — NNUE uses int8/int16 for blazing fast eval. For WASM deployment in the Bevy game, quantized inference lets the AI think deeper in real-time.

### From AlphaGo/AlphaZero

4. **Dirichlet noise at root** — adds exploration during self-play, prevents policy collapse early in training.

5. **Virtual loss in parallel MCTS** — if running multi-threaded self-play, virtual loss prevents all threads from exploring the same path.

6. **Symmetry augmentation** — the board has rotational/reflective symmetries (player 1 scores columns, player 2 scores rows, so a 90-degree rotation + player swap is equivalent). Use this to multiply training data.

### From Poker AI (for hidden-hands variant)

7. **Belief state tracking** — maintain probability distribution over opponent hands given observed play history. See [hidden.md](hidden.md).

---

## Roadmap

### Phase 1 — Fast Game Engine in Rust
- Board state, move generation, scoring — all in Rust
- This becomes the self-play backbone (needs millions of games)
- Validate against existing Python implementation

### Phase 2 — Pure MCTS (no neural net)
- MCTS with random rollouts as baseline
- Will beat greedy/aggressive easily on its own
- Establishes the search framework and benchmarking infrastructure

### Phase 3 — Neural Network
- Small ResNet in PyTorch (or tch-rs)
- Bootstrap training on self-play data from Phase 2
- Export to ONNX for Rust inference

### Phase 4 — AlphaZero Loop
- Self-play → train → evaluate → promote cycle
- ~50-100 iterations to convergence
- This is where Elo climbs fast

### Phase 5 — NNUE Distillation (for deployment)
- Distill the AlphaZero network into an NNUE-style efficiently-updatable architecture
- Pair with alpha-beta search for deterministic, fast play
- Deploy in the Bevy game (native + WASM)
- The Stockfish playbook: train with deep search, deploy with efficient eval

### Phase 6 — Hidden Hands Variant
- Extend engine to support imperfect information mode
- Implement ISMCTS or belief-augmented AlphaZero
- Compare open vs hidden Elo curves
