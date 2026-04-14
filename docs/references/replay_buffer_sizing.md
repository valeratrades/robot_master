# Replay Buffer Sizing for AlphaZero-style Training

## Original Papers

### AlphaGo Zero (Silver et al., Nature 2017)
Buffer stores the **most recent 500,000 games** as a FIFO queue. With ~25,000 games generated per iteration over 200 iterations (~5M total), the window covers roughly **the most recent 20 iterations** at any point during training. This is the origin of the "20× games_per_iteration" rule of thumb.

### AlphaZero (Silver et al., Science 2018)
Buffer capacity not specified in the paper. Reimplementations (ELF OpenGo, LightZero) default to the AGZ figure of 500k games.

## Reimplementations & Scale-Down Studies

### ELF OpenGo (Facebook, 2019) - arxiv 1902.04522
Explicitly confirmed: 500,000-game buffer in a split architecture of 50 queues × 10,000 games each. Added a **minimum floor of 200 games per queue** before training begins to prevent early overfitting. This floor heuristic is useful for small-scale runs.

### MiniZero (IEEE ToG 2023/2024) - arxiv 2310.11305
**Most directly applicable reference for low-compute regimes.**
- 300 iterations × 2,000 games/iter → 40,000-game buffer = exactly 20× games_per_iteration
- Independently reproduces the ~20× ratio at modest scale
- Confirms the ratio holds across orders of magnitude in compute

## Experience Replay Theory

### Revisiting Fundamentals of Experience Replay (Fedus et al., ICML 2020) - arxiv 2007.06700
Buffer size interacts nonlinearly with algorithm choice and n-step returns. "Larger buffer generally helps" when paired with appropriate algorithmic choices. No universal formula exists, but confirms that aggressive pruning (< ~10 iterations of history) causes variance spikes and slower convergence.

## Low-Compute / Sample Efficiency (2022-2025)

### Gumbel AlphaZero / Gumbel MuZero (Danihelka et al., ICLR 2022)
https://openreview.net/forum?id=bERaNdoegnO
Uses Gumbel-Top-k sampling and Sequential Halving to give valid policy improvement guarantees with **as few as 2 MCTS simulations per move**. MiniZero (2024) shows Gumbel AlphaZero with n=2 sims trains ~16× faster wall-clock than standard AlphaZero with n=200 sims, reaching similar strength. **Single highest-leverage improvement for compute-constrained training.**

### Search-Contempt (arxiv 2504.07757, April 2025)
Hybrid MCTS variant that preferentially generates self-play games from "challenging" positions rather than uniform game starts. Achieves up to **+70 Elo** with the same number of training games. Targets "standard consumer GPU with very limited compute budget." Does not address buffer sizing directly.

### AlphaZero-Inspired Game Learning: MCTS Only at Test Time (arxiv 2204.13307, 2022)
Removes MCTS from training entirely; uses it only at evaluation. Reached superhuman performance on Othello on standard CPU. Architectural alternative when compute is severely constrained.

## Key Takeaways for This Project

- The **~20× games_per_iteration** buffer cap is the empirical consensus, reproduced independently at both DeepMind scale and MiniZero's modest 300-iteration scale.
- `ln(N_iterations)` grows far too slowly - `ceil(ln(100)) = 5` iterations of history is more aggressive than any published implementation and risks policy divergence.
- Don't start gradient updates until the buffer has a meaningful floor (~500+ game records).
- If not already using Gumbel AlphaZero for MCTS, that is the highest-leverage algorithmic change for limited iteration budgets.
