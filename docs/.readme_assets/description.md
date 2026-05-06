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

## What's been built

- **Gumbel AlphaZero pipeline** — self-play in Rust, training in PyTorch, ONNX as the runtime contract. [Gumbel MuZero](https://openreview.net/forum?id=bERaNdoegnO) (Danihelka et al., ICLR 2022): works with 2–16 MCTS sims per move instead of the 400–800 vanilla AlphaZero needs.
- **Encoder-only transformer** — board cells as tokens, geometric attention bias ([Chessformer](https://arxiv.org/abs/2409.12272)), single model scales across board sizes 5×5 → 11×11 without retraining.
- **Arena** — Glicko-2 ratings, Swiss/round-robin/elimination tournaments. Trained ONNX models plug in as arena players against each other and built-in bots.
- **Bevy GUI + Leptos web app + TUI** — all const-generic over board size N ∈ {5, 7, 9, 11}.
- **Hidden-hands variant** — opponent's hand is hidden; engine and self-play support it, dedicated training and search not yet done.

## What's next

- **Alpha-beta search** with the transformer as eval — LMR, null-move pruning, aspiration windows. Stockfish-style depth where MCTS gives breadth.
- **WDL heads** — explicit win/draw/loss output instead of scalar value. Draws matter here and the scalar collapses them.
- **Imperfect information search** for the hidden-hands variant: ISMCTS baseline → belief-augmented transformer input → [ReBeL](https://arxiv.org/abs/2007.13544) / [Student of Games](https://www.science.org/doi/10.1126/sciadv.adg3256) (Schmid et al., 2023).

See [`docs/research.md`](docs/research.md) for details.

## Rules

1v1 on a 5×5 grid. Cards are numbered 0–5, with 6 copies each (36 total). Each player gets 12; a 25th card is placed at the center.

**Turns**: players alternate placing a card from their hand onto an empty cell adjacent (no diagonals) to an occupied one.

**Scoring** (per line/column, once the grid is full):
| copies of a card | points |
|---|---|
| 1 | face value |
| 2 | 10 × face value |
| 3+ | 100 flat |

**Winner**: Alice's score = her lowest-scoring column; Bob's score = his lowest-scoring row. Highest score wins.
