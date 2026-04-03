# AlphaZero/AlphaGo Zero: Temperature Sampling & Dirichlet Noise

Sources:
- **AlphaGo Zero**: Silver et al., Nature 2017 — authoritative source for all formulas
- **AlphaZero**: Silver et al., arXiv 1712.01815 — explicitly says "training and search algorithm and parameters are identical to AlphaGo Zero", adds only per-game Dirichlet α values

---

## Temperature Sampling

Move selection at the root after MCTS completes:

```
π(a | s₀) = N(s₀, a)^(1/τ) / Σ_b N(s₀, b)^(1/τ)
```

Then sample a move from distribution π.

**Schedule (AlphaGo Zero, "Self-Play" section, p. 24):**
> "For the first 30 moves of each game, the temperature is set to τ = 1; this selects moves proportionally to their visit count in MCTS, and ensures a diverse set of positions are encountered. For the remainder of the game, an infinitesimal temperature is used, τ → 0."

- τ = 1 for moves 0–29: sample ∝ visit counts (reduces to direct proportional sampling)
- τ → 0 for move 30+: argmax (greedy)

Note: at τ = 1 the formula simplifies to just normalizing visit counts directly (N / ΣN), no need to exponentiate.

---

## Dirichlet Noise

Applied **only at the root node**, to the prior probabilities, before any simulation:

```
P(s, a) = (1 - ε) * p_a  +  ε * η_a
where η ~ Dir(α)
```

**AlphaGo Zero ("Self-Play" section, p. 24):**
> "P(s, a) = (1 − ε)pₐ + εηₐ, where η ∼ Dir(0.03) and ε = 0.25; this noise ensures that all moves may be tried, but the search may still overrule bad moves."

### α values by game (AlphaZero, "Configuration" section, p. 14):
> "Dirichlet noise Dir(α) was added to the prior probabilities in the root node; this was scaled in inverse proportion to the approximate number of legal moves in a typical position, to a value of α = {0.3, 0.15, 0.03} for chess, shogi and Go respectively."

| Game | Avg legal moves | α    |
|------|----------------|------|
| Go   | ~250           | 0.03 |
| Shogi| ~80            | 0.15 |
| Chess| ~35            | 0.30 |
| **Robot Master 5x5** | **~25** | **~0.3** |

ε = 0.25 is the same across all games.

---

## PUCT Formula (for completeness)

```
a = argmax_a [ Q(s, a) + U(s, a) ]
U(s, a) = c_puct · P(s, a) · √(Σ_b N(s, b)) / (1 + N(s, a))
```

---

## Implementation Notes

- Dirichlet noise is applied once per move, to the root edges, before the simulation loop starts.
- Temperature affects only which move is *played* (sampled from π), not the MCTS simulations themselves.
- The policy target stored in training data is π (the full visit count distribution), not the sampled move.
- Tree reuse between moves: AlphaGo Zero reuses the subtree rooted at the played move. Optional optimization.
