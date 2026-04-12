# ResTNet (arXiv 2410.05347v2)

**Link:** https://arxiv.org/html/2410.05347v2

## Architecture

CNN + Transformer hybrid. Interleaves residual blocks (conv) with Transformer blocks. Optimal config: **R3(RRT)** — two residual blocks then one Transformer block, repeated. Residual blocks: standard AlphaZero (two conv layers, 256 filters). Transformer blocks: relative position encoding, 4 attention heads.

**Feature conversion:** 2D feature maps → 1D tokens via one-to-one positional mapping (row-major or column-major), preserving spatial correspondence.

## What Attention Learns

Without explicit programming, attention heads capture domain-specific concepts:
- Life-and-death recognition (Go)
- Uncertain territory identification
- Ladder pattern recognition
- Hex virtual connections

Attention maps correspond closely to known game knowledge concepts.

## Search

Gumbel AlphaZero MCTS. 64 sims for 9×9 Go, 32 sims for 19×19 Hex.

## Performance

- 9×9 Go: 54.6% → 60.8% win rate over baseline
- 19×19 Go: 53.6% → 60.9%
- 19×19 Hex: 50.4% → 58.0%
- Cyclic-adversary vulnerability: 70.44% → 23.91% attack success rate
- Ladder recognition: 59.15% → 80.01% accuracy

## Relevance to us

- **Most directly relevant to our architecture**: we already have a CNN stem + transformer body (model_transformer.py) which is the same hybrid idea
- The R3(RRT) interleaving pattern is different from our "full stem then full transformer" — worth considering
- Attention learning game concepts (lines, threats) mirrors what we'd want for Robot Master's scoring lines
- Gumbel AlphaZero MCTS — same framework we're using
- No NNUE/incremental eval
- No explicit activation function details reported
