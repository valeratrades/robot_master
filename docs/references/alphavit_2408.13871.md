# AlphaViT (arXiv 2408.13871)

**Link:** https://arxiv.org/html/2408.13871

## Core Idea

Replaces AlphaZero's ResNet residual blocks with Vision Transformer (ViT) components. Handles variable board sizes with a single model - AlphaZero's ResNet requires fixed input size, ViT doesn't.

## Three Variants

- **AlphaViT**: encoder only. Three special learnable embeddings: value token (state eval), game token (which game), pass token. Board divided into patches via conv layer, projected to embeddings.
- **AlphaViD**: encoder + decoder. Decoder inputs derived from encoder outputs via FC layer.
- **AlphaVDA**: encoder + decoder with learnable action embeddings as decoder inputs. Most flexible action space adaptation.

## Variable Board Size

Patch-based: `n_patches = f(H, W, patch_size, stride, padding)`. Position embeddings scaled by board size. AlphaViD/VDA use bilinear interpolation to resize embeddings to desired action space size.

## Search

Standard AlphaZero MCTS with UCT. Identical to AlphaZero's MCTS - no modifications.

## Training

AlphaZero's three-stage loop: self-play → augmentation → update. 1000 iterations. Multi-GPU data parallelization, automatic mixed precision (RTX 4060 Ti / 3060).

## Performance

Tested on Connect 4, Gomoku, Othello. AlphaViT L4 approaches AlphaZero within ~270 Elo. Multitask training (all three games simultaneously) competitive with single-game training, especially on large boards. Pre-training on small boards then fine-tuning accelerates convergence.

## Relevance to us

- Directly relevant: we already have a transformer model (model_transformer.py) - AlphaViT validates this direction
- Variable board size via patch embeddings is cleaner than our current fixed-size approach
- Multitask / variable-N training is something we haven't explored
- No NNUE or incremental eval discussion - pure NN + MCTS stack
