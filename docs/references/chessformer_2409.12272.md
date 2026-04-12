# Chessformer (arXiv 2409.12272v2)

**Link:** https://arxiv.org/html/2409.12272v2

## Architecture

Transformer encoder-only. Largest model (CF-240M): 15 encoder layers, embedding depth 1024, 32 heads, feedforward depth 4096.

**Activation:** Mish in feedforward sublayers throughout encoder body.

**Position encoding:** Shaw et al. method — learnable vectors modify both attention logits and output aggregation (captures spatial relationships like diagonal moves, not just Euclidean proximity). Compared against absolute embeddings and relative biases; Shaw won.

## Training

Supervised from static self-play dataset (500M games for CF-240M). Multiple auxiliary targets: policy (vanilla + soft temperature), value heads (game result, L2 reward, categorical, error prediction). Post-LN normalization with DeepNorm init. Stochastic weight averaging for final checkpoints.

## Search

**No MCTS.** Two agent types:
- Policy agents: pick highest-ranked move from policy vector (1 eval)
- Value agents: evaluate all legal moves, pick max (~20 evals per position)

Avoids search entirely, relies on direct network predictions.

## Relevance to us

- No incremental eval / NNUE discussion
- Mish activation is an alternative to our leaky_relu/SCReLU stack — not relevant unless we go pure transformer
- Shaw position encoding is interesting for our transformer model's spatial bias
- The "no MCTS" approach is a data point for how far pure network quality can get you
