"""Track B: Transformer with Block Attention Residuals for AlphaZero.

Architecture (N×N board):
  Input:  in_channels(N) x N x N
  Stem:   Conv(in_ch->d_model, 3x3) -> BN -> ReLU -> num_stem_blocks x SE-ResBlock(d_model)
  Tokens: flatten spatial dims -> (B, N*N, d_model)
  Body:   num_layers x TransformerLayer with Block AttnRes residuals
  Policy: reshape -> (B, d_model, N, N) -> Conv(->2, 1x1) -> BN -> ReLU -> FC(2*N*N, action_size)
  Value:  reshape -> (B, d_model, N, N) -> Conv(->1, 1x1) -> BN -> ReLU -> FC(N*N, 64) -> ReLU -> FC(64, 1) -> tanh

Block AttnRes (arxiv 2603.15031):
  Replace standard h = h_prev + f(h_prev) with:
    attended = softmax_over_depth(query @ [b_0,...,b_{n-1}, partial_block])
    h = partial_block + f(attended)  -- but since we use "bias" gate type, attended replaces partial_block input
  The recency_bias is a large scalar added to the last logit (partial_block), so at init
  attended ≈ partial_block and the model is equivalent to a standard transformer.
  During training, proj weights learn to blend information from earlier blocks.
"""

import torch
import torch.nn as nn
import torch.nn.functional as F

from model_resnet import SEBlock, ResBlock, in_channels, action_size


# ---------------------------------------------------------------------------
# Block AttnRes core operation
# ---------------------------------------------------------------------------

def block_attn_res(
    blocks: list[torch.Tensor],   # completed block sums [B, T, D] each
    partial_block: torch.Tensor,  # current intra-block partial sum [B, T, D]
    proj: nn.Linear,              # pseudo-query weight, Linear(D, 1, bias=False)
    norm: nn.RMSNorm,             # applied to keys before scoring
    recency_bias: nn.Parameter,   # scalar added to partial_block's logit
) -> torch.Tensor:
    """Attend over all block representations + current partial block.

    Returns [B, T, D] — the attended aggregation of depth history.
    At init (proj weights zero, large recency_bias), output ≈ partial_block.
    """
    # Stack: [N+1, B, T, D]
    V = torch.stack(blocks + [partial_block], dim=0)

    # Keys = RMSNorm(V)
    K = norm(V)

    # Pseudo-query: single learned vector (D,)
    query = proj.weight.view(-1)                              # (D,)
    logits = torch.einsum("d, n b t d -> n b t", query, K)   # (N+1, B, T)

    # Recency bias: boost partial_block (last element)
    logits[-1] = logits[-1] + recency_bias

    weights = logits.softmax(dim=0)                           # (N+1, B, T)
    h = torch.einsum("n b t, n b t d -> b t d", weights, V)  # (B, T, D)
    return h


# ---------------------------------------------------------------------------
# Transformer layer with Block AttnRes
# ---------------------------------------------------------------------------

class AttnResTransformerLayer(nn.Module):
    """Single transformer layer (self-attn + MLP) using Block AttnRes residuals.

    At each sublayer, instead of computing f(partial_block), we compute:
      h = block_attn_res(blocks, partial_block)   # ≈ partial_block at init
      sublayer_out = f(LayerNorm(h))
      partial_block = partial_block + sublayer_out

    This preserves the standard residual stream semantics while allowing each
    sublayer to selectively aggregate from earlier block representations.
    """

    def __init__(self, d_model: int, num_heads: int, mlp_ratio: int, recency_bias_init: float, layer_idx: int, layers_per_block: int):
        super().__init__()
        self.layer_idx = layer_idx
        self.layers_per_block = layers_per_block

        # Self-attention sublayer
        self.attn = nn.MultiheadAttention(d_model, num_heads, batch_first=True)
        self.norm1 = nn.LayerNorm(d_model)

        # MLP sublayer
        d_ff = d_model * mlp_ratio
        self.mlp = nn.Sequential(
            nn.Linear(d_model, d_ff),
            nn.GELU(),
            nn.Linear(d_ff, d_model),
        )
        self.norm2 = nn.LayerNorm(d_model)

        # AttnRes components for attention sublayer
        self.attn_res_proj = nn.Linear(d_model, 1, bias=False)
        self.attn_res_norm = nn.RMSNorm(d_model)
        self.attn_res_bias = nn.Parameter(torch.tensor(recency_bias_init))

        # AttnRes components for MLP sublayer
        self.mlp_res_proj = nn.Linear(d_model, 1, bias=False)
        self.mlp_res_norm = nn.RMSNorm(d_model)
        self.mlp_res_bias = nn.Parameter(torch.tensor(recency_bias_init))

        # Zero-init pseudo-query projections: uniform attention across blocks at init,
        # but recency_bias >> 0 dominates, making attended ≈ partial_block.
        nn.init.zeros_(self.attn_res_proj.weight)
        nn.init.zeros_(self.mlp_res_proj.weight)

    @property
    def is_block_boundary(self) -> bool:
        return (self.layer_idx + 1) % self.layers_per_block == 0

    def forward(
        self,
        blocks: list[torch.Tensor],
        partial_block: torch.Tensor,
    ) -> tuple[list[torch.Tensor], torch.Tensor]:
        # ---- Attention sublayer ----
        h = block_attn_res(blocks, partial_block, self.attn_res_proj, self.attn_res_norm, self.attn_res_bias)
        attn_out, _ = self.attn(self.norm1(h), self.norm1(h), self.norm1(h))
        partial_block = partial_block + attn_out

        # ---- MLP sublayer ----
        h = block_attn_res(blocks, partial_block, self.mlp_res_proj, self.mlp_res_norm, self.mlp_res_bias)
        mlp_out = self.mlp(self.norm2(h))
        partial_block = partial_block + mlp_out

        # At block boundaries, snapshot partial_block into the history
        if self.is_block_boundary:
            blocks = blocks + [partial_block]

        return blocks, partial_block


# ---------------------------------------------------------------------------
# Full model
# ---------------------------------------------------------------------------

class RobotMasterTransformer(nn.Module):
    """AlphaZero dual-headed Transformer with Block AttnRes for Robot Master.

    Drop-in replacement for RobotMasterResNet:
      forward(x) -> (policy_logits, value_scalar)

    Architecture:
      - Stem: Conv + BN + ReLU + num_stem_blocks x SE-ResBlock  [spatial encoding]
      - Flatten: (B, d_model, N, N) -> (B, N*N, d_model)        [token sequence]
      - Transformer: num_layers x AttnResTransformerLayer        [depth attention]
      - Reshape: (B, N*N, d_model) -> (B, d_model, N, N)        [spatial recovery]
      - Policy head and value head (same as ResNet)
    """

    def __init__(
        self,
        board_size: int = 5,
        num_stem_blocks: int = 3,
        num_filters: int = 64,
        num_transformer_layers: int = 8,
        num_attnres_blocks: int = 4,
        num_heads: int = 4,
        mlp_ratio: int = 4,
        recency_bias_init: float = 3.0,
    ):
        super().__init__()
        self.board_size = board_size
        self.d_model = num_filters
        n2 = board_size * board_size

        # ---- Stem: spatial feature extraction ----
        se_channels = max(num_filters // 8, 4)
        self.conv_in = nn.Conv2d(in_channels(board_size), num_filters, 3, padding=1, bias=False)
        self.bn_in = nn.BatchNorm2d(num_filters)
        self.stem_blocks = nn.ModuleList([ResBlock(num_filters, se_channels) for _ in range(num_stem_blocks)])

        # ---- Transformer body with Block AttnRes ----
        layers_per_block = max(1, (num_transformer_layers + num_attnres_blocks - 1) // num_attnres_blocks)
        self.transformer_layers = nn.ModuleList([
            AttnResTransformerLayer(
                d_model=num_filters,
                num_heads=num_heads,
                mlp_ratio=mlp_ratio,
                recency_bias_init=recency_bias_init,
                layer_idx=i,
                layers_per_block=layers_per_block,
            )
            for i in range(num_transformer_layers)
        ])
        self.final_norm = nn.LayerNorm(num_filters)

        # ---- Policy head ----
        self.policy_conv = nn.Conv2d(num_filters, 2, 1, bias=False)
        self.policy_bn = nn.BatchNorm2d(2)
        self.policy_fc = nn.Linear(2 * n2, action_size(board_size))

        # ---- Value head ----
        self.value_conv = nn.Conv2d(num_filters, 1, 1, bias=False)
        self.value_bn = nn.BatchNorm2d(1)
        self.value_fc1 = nn.Linear(n2, 64)
        self.value_fc2 = nn.Linear(64, 1)

    def forward(self, x: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        B, _, N, _ = x.shape

        # ---- Stem ----
        out = F.relu(self.bn_in(self.conv_in(x)))
        for block in self.stem_blocks:
            out = block(out)
        # out: (B, d_model, N, N)

        # ---- Flatten to token sequence ----
        # (B, d_model, N, N) -> (B, N*N, d_model)
        tokens = out.flatten(2).transpose(1, 2)

        # ---- Transformer with Block AttnRes ----
        # Token embedding acts as b_0 per the paper (§3.2, b_0 = h_1 = token embedding)
        blocks: list[torch.Tensor] = [tokens]
        partial_block: torch.Tensor = tokens

        for layer in self.transformer_layers:
            blocks, partial_block = layer(blocks, partial_block)

        partial_block = self.final_norm(partial_block)

        # ---- Reshape back to spatial ----
        # (B, N*N, d_model) -> (B, d_model, N, N)
        spatial = partial_block.transpose(1, 2).reshape(B, self.d_model, N, N)

        # ---- Policy head ----
        p = F.relu(self.policy_bn(self.policy_conv(spatial)))
        p = p.flatten(1)
        p = self.policy_fc(p)

        # ---- Value head ----
        v = F.relu(self.value_bn(self.value_conv(spatial)))
        v = v.flatten(1)
        v = F.relu(self.value_fc1(v))
        v = torch.tanh(self.value_fc2(v))

        return p, v.squeeze(-1)


if __name__ == "__main__":
    from model_resnet import encode_state

    board_size = 5
    model = RobotMasterTransformer(board_size=board_size)
    total_params = sum(p.numel() for p in model.parameters())
    print(f"Parameters: {total_params:,}")

    batch = torch.randn(4, in_channels(board_size), board_size, board_size)
    model.eval()
    with torch.no_grad():
        policy, value = model(batch)

    assert policy.shape == (4, action_size(board_size)), f"policy shape: {policy.shape}"
    assert value.shape == (4,), f"value shape: {value.shape}"
    assert (value.abs() <= 1.0).all(), "value out of [-1, 1]"
    print(f"Policy shape: {policy.shape}  (expected (4, {action_size(board_size)}))")
    print(f"Value shape:  {value.shape}  (expected (4,))")
    print(f"Value range:  [{value.min():.4f}, {value.max():.4f}]")
    print("All checks passed.")
