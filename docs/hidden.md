# Hidden Hands Variant: Imperfect Information

Making hands hidden transforms Robot Master from a Go/Chess-like game into a Poker/Bridge-like game. This doesn't add randomness to game mechanics — it adds **uncertainty about opponent state**.

## What Changes Strategically

- Can't calculate "opponent has two 5s" — must **estimate** from play history
- Bluffing: placing a 3 might signal more 3s, or might be a fake
- Card counting: tracking what's played constrains what's possible in their hand
- Search shifts from minimax to **belief-state search** (probability distribution over opponent hands)

## Why It's Not Just Luck

Poker proved this. Texas Hold'em has far more hidden information, yet Libratus/Pluribus crush humans. The skill ceiling in imperfect-info games is often *higher* — you optimize over beliefs, not just positions.

For Robot Master specifically:
- 36 cards total, 25 get played on board — by mid-game you've seen ~12 and can narrow opponent's hand significantly
- Scoring nonlinearity (2 copies = 10x, 3+ = 100) makes correct inference about holdings hugely rewarding
- Only 6 distinct card values — information space is tractable but non-trivial

## The Complexity Spectrum

| Variant | Information | Search Paradigm | Analog |
|---------|------------|-----------------|--------|
| Open hands | Perfect | AlphaZero / minimax | Go, Chess |
| Hidden hands | Imperfect | CFR / ISMCTS | Poker, Bridge |
| Hidden + random draw per turn | Imperfect + stochastic | Deep CFR | Poker (closer) |

## Approaches

### Option A — Information Set MCTS (ISMCTS)

Sample possible opponent hands consistent with observations, run MCTS on each "determinization", aggregate results.

**Pros:**
- Simple to implement on top of existing MCTS infrastructure
- Works surprisingly well for moderate hidden information
- How strong Bridge AIs work

**Cons:**
- Strategy fusion problem: aggregating over determinizations can produce moves that are good on average but terrible in specific worlds
- Doesn't learn to bluff or exploit opponent tendencies

**Best for:** first imperfect-info baseline, getting something strong quickly.

### Option B — Counterfactual Regret Minimization (CFR)

The Poker approach. Iteratively minimizes regret across all information sets to converge on Nash equilibrium — unexploitable play.

**Pros:**
- Theoretically optimal (converges to Nash equilibrium)
- Produces balanced, unexploitable strategies
- Well-understood convergence guarantees

**Cons:**
- Heavy to implement properly
- Information set space might be large (all possible hand/board combos)
- Likely overkill for this game size
- Doesn't exploit weak opponents (plays equilibrium, not best-response)

**Best for:** if you want provably unexploitable play and are willing to invest in implementation.

### Option C — AlphaZero + Belief Network

Add a **hand prediction head** to the AlphaZero network: given board history, predict opponent's hand distribution. Feed predicted hand into value/policy evaluation.

```
Input planes (board + own hand + play history)
        │
   ┌────┴────┐
   │ ResNet  │
   │ trunk   │
   └────┬────┘
   ┌────┼────────┐
   │    │        │
Policy Value  Belief
 head   head   head
   │    │        │
   π    v    P(opponent hand)
```

**Pros:**
- Natural extension of the open-hands AlphaZero pipeline
- Learns to infer opponent holdings from play patterns
- Can implicitly learn to bluff (policy learns that some moves mislead opponent's belief model)
- Single unified training loop

**Cons:**
- No equilibrium guarantees (could be exploitable)
- Belief head accuracy is a bottleneck
- Needs play history as additional input (sequence of moves, not just current board)

**Best for:** maximum Elo if you already have the AlphaZero infrastructure, and the most interesting ML angle.

## Recommendation

Start with **Option A (ISMCTS)** as the first imperfect-info agent — it builds directly on the open-hands MCTS work. Then pursue **Option C (belief-augmented AlphaZero)** for the strongest possible agent. Skip Option B unless you specifically want game-theoretic optimality.

Comparing open-hands AlphaZero vs hidden-hands belief-augmented AlphaZero Elo curves would be a paper-worthy result on its own.
