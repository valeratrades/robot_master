# Gumbel AlphaZero: Implementation Reference

**Paper:** "Policy Improvement by Planning with Gumbel" — Danihelka, Guez, Schrittwieser, Silver. ICLR 2022.
https://openreview.net/forum?id=bERaNdoegnO

Reference implementation: https://github.com/deepmind/mctx

---

## What changes vs standard AlphaZero

| AlphaZero | Gumbel AlphaZero |
|---|---|
| Dirichlet noise at root | Gumbel-Top-k sampling (no replacement) |
| PUCT at root | Sequential Halving with Gumbel scores |
| Sample action from annealed visit-count distribution | Deterministic argmax of A_{n+1} from Sequential Halving |
| Policy target = visit count distribution | Policy target = softmax of completed Q-values |

Non-root nodes: still standard PUCT. The "Full Gumbel" variant replaces those too (Eq. 14), but the paper shows it gives only a small benefit — base Gumbel is the recommended default.

---

## Step 1: Sample Gumbel noise (once per move)

```
g ~ Gumbel(0)^k    # k = number of legal actions at root
```

Gumbel(0) CDF: F(x) = exp(-exp(-x)). Sample via inverse CDF: g = -log(-log(u)), u ~ Uniform(0,1).

---

## Step 2: Select top-m actions without replacement

```
A_topm = argtop(g + logits, m)
```

where `logits = log π(a)` (log-probabilities from network policy head).
Default: `m = min(n, 16)`.

---

## Step 3: Sequential Halving (allocates n simulations over A_topm)

Phases = ceil(log2(m)). Each phase:
1. Each surviving action gets `floor(n / (phases * |survivors|))` new simulations (≥ 1)
2. Each simulation = one standard MCTS rollout (PUCT at non-root, expand leaf, backup)
3. After phase: rank survivors by `g(a) + logits(a) + σ(q̂(a))`, drop bottom half

Final action: last survivor = A_{n+1}. If budget exhausted early, pick argmax from survivors.

**Example** (m=16, n=200, phases=4):
- Phase 1: 16 actions × 3 visits = 48
- Phase 2: 8 actions × 6 visits = 48
- Phase 3: 4 actions × 12 visits = 48
- Phase 4: 2 actions × 25 visits = 50 → total ≈ 194, remainder fills last phase

---

## σ function (Q-value scaling, Eq. 8)

```
σ(q̂(a)) = (c_visit + max_b N(b)) * c_scale * q̂(a)
```

- c_visit = 50, c_scale = 1.0 (robust across sim counts)
- q̂(a) normalized to [0,1]: `(q - min) / (max - min)` using min/max seen in tree

---

## Completed Q-values (Eq. 10)

Unvisited actions get assigned the mixed value estimate (not zero):

```
completedQ(a) = q̂(a)      if N(a) > 0
              = v_mix      otherwise
```

```
v_mix = (v̂_π + sum_{N(a)>0} π(a)*q̂(a) / sum_{N(a)>0} π(a)) / (1 + sum_b N(b))
      * (1 + sum_b N(b))
```

Simplified (Appendix D, Eq. 33): interpolation between network value v̂_π and mean of observed Q-values weighted by prior:

```
v_mix = (v̂_π + Σ_{a:N(a)>0} π(a)*q̂(a)) / (1 + Σ_{a:N(a)>0} π(a))
```

---

## Policy target (training data, Eqs. 11-12)

```
π' = softmax(logits + σ(completedQ))   # improved policy
```

Store π' as the policy target (replaces visit-count distribution).
Training loss: `KL(π' || π_network)` instead of cross-entropy on visit counts.

This is the key training data change — all actions get a meaningful gradient, not just visited ones.

---

## Parameters

| Parameter | Value | Notes |
|---|---|---|
| m | min(n, 16) | actions sampled without replacement |
| n | 16–32 for training | works at n=2; evaluated at n=800 |
| c_visit | 50 | σ scaling constant |
| c_scale | 1.0 | σ scaling constant (0.1 for unnormalized rewards) |
| phases | ceil(log2(m)) | Sequential Halving phases |

For Robot Master 5x5 (~25 legal moves): m = min(n, 16) is sensible. With n=16, m=16 — no halving needed, just visit all m and pick argmax.

---

## Special case: n ≥ k (small action spaces)

When total simulations ≥ number of legal actions, just visit every action once and pick argmax. No Sequential Halving needed. This is the Robot Master case at small n.

---

## What to change in training data format

Current: store visit-count distribution (Vec<f32> of length 6*N²).
After Gumbel: store π' = softmax(logits + σ(completedQ)) — same shape, different values.
Training loss in Python: KL divergence instead of cross-entropy (or keep cross-entropy — KL and CE differ only by a constant w.r.t. network params).
