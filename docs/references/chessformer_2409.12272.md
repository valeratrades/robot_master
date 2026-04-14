# Chessformer (arXiv 2409.12272v2)

**Link:** https://arxiv.org/html/2409.12272v2

## Architecture

Transformer encoder-only. Largest model (CF-240M): 15 encoder layers, embedding depth 1024, 32 heads, feedforward depth 4096.

**Activation:** Mish in feedforward sublayers throughout encoder body.

**Position encoding:** Shaw et al. method - learnable vectors modify both attention logits and output aggregation (captures spatial relationships like diagonal moves, not just Euclidean proximity). Compared against absolute embeddings and relative biases; Shaw won.

## Training

Supervised from static self-play dataset (500M games for CF-240M). Multiple auxiliary targets: policy (vanilla + soft temperature), value heads (game result, L2 reward, categorical, error prediction). Post-LN normalization with DeepNorm init. Stochastic weight averaging for final checkpoints.

## Search

**No MCTS.** Two agent types:
- Policy agents: pick highest-ranked move from policy vector (1 eval)
- Value agents: evaluate all legal moves, pick max (~20 evals per position)

Avoids search entirely, relies on direct network predictions.

## Relevance to us

- No incremental eval / NNUE discussion
- Mish activation is an alternative to our leaky_relu/SCReLU stack - not relevant unless we go pure transformer
- Shaw position encoding is interesting for our transformer model's spatial bias
- The "no MCTS" approach is a data point for how far pure network quality can get you

# Could Steal
1. Shaw Relative Position Encoding (RPE) - Most Critical

If we ever move from SE-ResNet to a transformer backbone, Shaw RPE is the right choice, not absolute embeddings. The paper demonstrates a 1.83% policy accuracy gain from RPE vs. absolute.

For our game specifically, RPE is compelling because:
- Board adjacency rules are local (moves must be adjacent squares)
- Player A scores columns, Player B scores rows - RPE captures this 2D asymmetry
- This could potentially replace the board-transpose trick we use to make the game player-invariant

Shaw's method adds learnable relative position vectors a_ij^Q, a_ij^K, a_ij^V that modify both attention logits and value aggregation - not just positional embeddings added to tokens.

2. Multi-Head Auxiliary Value Targets

Currently we train: policy + scalar value. Chessformer trains:
- W/D/L categorical outcome
- L2 reward (soft value)
- Categorical value (32 buckets)
- Error prediction (meta-head predicting network confidence)
- Moves-left head

We could add:
- Per-line score heads - predict each player's scoring for individual rows/columns (aligns perfectly with our scoring structure)
- Game-length head (moves left)
- Categorical value instead of scalar

3. Soft Policy Targets

Instead of hard one-hot targets (the move MCTS chose), use a temperature-softened distribution over all MCTS visit counts. Chessformer weights this loss 8× higher than the hard policy loss. This reduces overfitting and trains on the full
visit distribution, not just the argmax.

4. Stochastic Weight Averaging (SWA)

Simple post-training averaging of checkpoints improves generalization. Zero architectural cost, easy to add to our training loop.

5. Diff-Focus Sampling

From chunkparser.py: probabilistically skip "easy" positions (where Q-delta between best and second-best move is small). This focuses compute on positions where the network is actually uncertain. Easy addition to our replay buffer
sampling.

---
What We Should NOT Take

- Search-free playing: Chessformer skips MCTS entirely, but that's only viable at 500M games of scale. We're nowhere near that; our Gumbel MCTS is still essential.
- Supervised training paradigm: They train on a pre-existing database; we do online self-play. Different approach.
