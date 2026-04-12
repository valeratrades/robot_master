"""Track B: Transformer with Block Attention Residuals for AlphaZero.

Architecture (N×N board):
  Input:  in_channels(N) x N x N
  Stem:   Conv(in_ch->d_model, 3x3) -> BN -> ReLU -> num_stem_blocks x SE-ResBlock(d_model)
  Tokens: flatten spatial dims -> (B, N*N, d_model)
  Body:   num_layers x TransformerLayer with Block AttnRes residuals
  Policy: reshape -> (B, d_model, N, N) -> Conv(->2, 1x1) -> BN -> ReLU -> FC(2*N*N, action_size)
  Value:  reshape -> (B, d_model, N, N) -> Conv(->1, 1x1) -> BN -> ReLU -> FC(N*N, 64) -> ReLU -> FC(64, 1) -> tanh

Block AttnRes (arxiv 2603.15031):
  Replace standard h = h_prev + f(h_prev) with softmax attention over block-level history:
    attended = softmax_over_depth(query @ [b_0, ..., b_{n-1}, partial_block])
    sublayer_out = f(LayerNorm(attended))
    partial_block = partial_block + sublayer_out
  The recency_bias is a large scalar added to the last logit (partial_block), so at init
  attended ≈ partial_block and the model behaves identically to a standard transformer.

ONNX compatibility:
  Block history is stored in a pre-allocated tensor buf[max_blocks, B, T, D].
  Each layer receives a compile-time-constant slice count `n_blocks_seen` so that
  buf[:n_blocks_seen+1] is a static shape from the ONNX tracer's perspective.
  This avoids dynamic Python lists and variable-length stacks that break tracing.
"""

import torch
import torch.nn as nn
import torch.nn.functional as F

from model_resnet import SEBlock, ResBlock, in_channels, action_size, screlu


# ---------------------------------------------------------------------------
# RMSNorm (manual — RMSNorm not supported by ONNX opset 18)
# ---------------------------------------------------------------------------

class RMSNorm(nn.Module):
    def __init__(self, d: int, eps: float = 1e-6):
        super().__init__()
        self.eps = eps
        self.weight = nn.Parameter(torch.ones(d))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        rms = x.pow(2).mean(-1, keepdim=True).add(self.eps).sqrt()
        return self.weight * (x / rms)


# ---------------------------------------------------------------------------
# Block AttnRes core operation
# ---------------------------------------------------------------------------

def block_attn_res(
    V: torch.Tensor,              # stacked history [N, B, T, D] — blocks + partial
    proj: nn.Linear,              # pseudo-query weight, Linear(D, 1, bias=False)
    norm: RMSNorm,             # applied to keys before scoring
    recency_bias: nn.Parameter,   # scalar added to partial_block's logit (last slot)
) -> torch.Tensor:
    """Attend over N block representations (last entry = current partial block).

    Returns [B, T, D] — the attended aggregation of depth history.
    At init (proj weights zero, large recency_bias), output ≈ V[-1] = partial_block.
    """
    # Keys = RMSNorm(V), shape [N, B, T, D]
    K = norm(V)

    # Pseudo-query: single learned vector (D,)
    query = proj.weight.view(-1)                              # (D,)
    logits = torch.einsum("d, n b t d -> n b t", query, K)   # (N, B, T)

    # Recency bias: boost last slot (partial_block)
    bias = torch.zeros(logits.shape[0], 1, 1, device=logits.device, dtype=logits.dtype)
    bias[-1] = recency_bias
    logits = logits + bias

    weights = logits.softmax(dim=0)                           # (N, B, T)
    h = torch.einsum("n b t, n b t d -> b t d", weights, V)  # (B, T, D)
    return h


# ---------------------------------------------------------------------------
# Transformer layer with Block AttnRes
# ---------------------------------------------------------------------------

class AttnResTransformerLayer(nn.Module):
    """Single transformer layer (self-attn + MLP) using Block AttnRes residuals.

    `n_blocks_before` is a compile-time constant: how many completed blocks exist
    before this layer runs. The block buffer slice buf[:n_blocks_before+1] has a
    static shape that ONNX can trace without dynamic control flow.
    """

    def __init__(
        self,
        d_model: int,
        num_heads: int,
        mlp_ratio: int,
        recency_bias_init: float,
        is_boundary: bool,
        n_blocks_before: int,
    ):
        super().__init__()
        self.is_boundary = is_boundary
        self.n_blocks_before = n_blocks_before

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
        self.attn_res_norm = RMSNorm(d_model)
        self.attn_res_bias = nn.Parameter(torch.tensor(recency_bias_init))

        # AttnRes components for MLP sublayer
        self.mlp_res_proj = nn.Linear(d_model, 1, bias=False)
        self.mlp_res_norm = RMSNorm(d_model)
        self.mlp_res_bias = nn.Parameter(torch.tensor(recency_bias_init))

        # Zero-init pseudo-queries: at init, recency_bias dominates → attended ≈ partial_block.
        nn.init.zeros_(self.attn_res_proj.weight)
        nn.init.zeros_(self.mlp_res_proj.weight)

    def forward(
        self,
        buf: torch.Tensor,        # [max_blocks, B, T, D] — pre-allocated block buffer
        next_slot: int,           # index of next free slot in buf (compile-time constant)
        partial_block: torch.Tensor,  # [B, T, D]
    ) -> tuple[torch.Tensor, int, torch.Tensor]:
        n = self.n_blocks_before  # number of completed blocks before this layer

        # Build V: [n+1, B, T, D] — n completed blocks + current partial
        # buf[:n] are the completed blocks; partial_block is appended as the last slot.
        # This slice has a static shape (n is compile-time constant).
        if n == 0:
            V_attn = partial_block.unsqueeze(0)   # [1, B, T, D]
        else:
            V_attn = torch.cat([buf[:n], partial_block.unsqueeze(0)], dim=0)

        # ---- Attention sublayer ----
        h = block_attn_res(V_attn, self.attn_res_proj, self.attn_res_norm, self.attn_res_bias)
        attn_out, _ = self.attn(self.norm1(h), self.norm1(h), self.norm1(h))
        partial_block = partial_block + attn_out

        # V for MLP: partial_block has been updated, rebuild with same blocks
        if n == 0:
            V_mlp = partial_block.unsqueeze(0)
        else:
            V_mlp = torch.cat([buf[:n], partial_block.unsqueeze(0)], dim=0)

        # ---- MLP sublayer ----
        h = block_attn_res(V_mlp, self.mlp_res_proj, self.mlp_res_norm, self.mlp_res_bias)
        mlp_out = self.mlp(self.norm2(h))
        partial_block = partial_block + mlp_out

        # At block boundaries, write partial_block into the next buffer slot
        if self.is_boundary:
            buf = buf.clone()
            buf[next_slot] = partial_block
            next_slot = next_slot + 1

        return buf, next_slot, partial_block


# ---------------------------------------------------------------------------
# Full model
# ---------------------------------------------------------------------------

class RobotMasterTransformer(nn.Module):
    """AlphaZero dual-headed Transformer with Block AttnRes for Robot Master.

    Drop-in replacement for RobotMasterResNet:
      forward(x) -> (policy_logits, value_scalar)
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
        self.num_attnres_blocks = num_attnres_blocks
        n2 = board_size * board_size

        # ---- Stem: spatial feature extraction ----
        se_channels = max(num_filters // 8, 4)
        self.conv_in = nn.Conv2d(in_channels(board_size), num_filters, 3, padding=1, bias=False)
        self.bn_in = nn.BatchNorm2d(num_filters)
        self.stem_blocks = nn.ModuleList([ResBlock(num_filters, se_channels) for _ in range(num_stem_blocks)])

        # ---- Transformer body with Block AttnRes ----
        layers_per_block = max(1, (num_transformer_layers + num_attnres_blocks - 1) // num_attnres_blocks)
        layers = []
        n_blocks_before = 0  # completed blocks before each layer (includes b_0 = token embedding)
        for i in range(num_transformer_layers):
            is_boundary = (i + 1) % layers_per_block == 0
            layers.append(AttnResTransformerLayer(
                d_model=num_filters,
                num_heads=num_heads,
                mlp_ratio=mlp_ratio,
                recency_bias_init=recency_bias_init,
                is_boundary=is_boundary,
                # b_0 = token embedding is always slot 0; completed transformer blocks start at slot 1
                n_blocks_before=n_blocks_before + 1,  # +1 for token embedding in slot 0
            ))
            if is_boundary:
                n_blocks_before += 1
        self.transformer_layers = nn.ModuleList(layers)
        self.final_norm = nn.LayerNorm(num_filters)

        # ---- Policy head ----
        self.policy_conv = nn.Conv2d(num_filters, 2, 1, bias=False)
        self.policy_bn = nn.BatchNorm2d(2)
        self.policy_fc = nn.Linear(4 * n2, action_size(board_size))  # 2x from SCReLU

        # ---- Value head ----
        self.value_conv = nn.Conv2d(num_filters, 1, 1, bias=False)
        self.value_bn = nn.BatchNorm2d(1)
        self.value_fc1 = nn.Linear(2 * n2, 64)  # 2x from SCReLU
        self.value_fc2 = nn.Linear(128, 1)  # 2x from SCReLU

    def forward(self, x: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        N = self.board_size

        # ---- Stem ----
        out = F.leaky_relu(self.bn_in(self.conv_in(x)))
        for block in self.stem_blocks:
            out = block(out)
        # out: (B, d_model, N, N)

        # ---- Flatten to token sequence ----
        # (B, d_model, N, N) -> (B, N*N, d_model)
        tokens = out.flatten(2).transpose(1, 2)

        B, T, D = tokens.shape

        # ---- Pre-allocate block buffer ----
        # Slots: [0] = token embedding (b_0), [1..] = completed transformer blocks
        max_slots = 1 + self.num_attnres_blocks
        buf = tokens.new_zeros(max_slots, B, T, D)
        buf[0] = tokens  # b_0 = token embedding
        next_slot = 1

        partial_block = tokens
        for layer in self.transformer_layers:
            buf, next_slot, partial_block = layer(buf, next_slot, partial_block)

        partial_block = self.final_norm(partial_block)

        # ---- Reshape back to spatial ----
        # (B, N*N, d_model) -> (B, d_model, N, N)
        spatial = partial_block.transpose(1, 2).reshape(-1, self.d_model, N, N)

        # ---- Policy head ----
        p = screlu(self.policy_bn(self.policy_conv(spatial)))
        p = p.flatten(1)
        p = self.policy_fc(p)

        # ---- Value head ----
        v = screlu(self.value_bn(self.value_conv(spatial)))
        v = v.flatten(1)
        v = screlu(self.value_fc1(v))
        v = torch.tanh(self.value_fc2(v))

        return p, v.squeeze(-1)


if __name__ == "__main__":
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
