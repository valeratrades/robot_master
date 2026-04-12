"""Track A: Small SE-ResNet for AlphaZero.

Architecture (N×N board):
  Input:  in_channels(N) x N x N   where in_channels(N) = 5*N + 5
  Body:   Conv(in_channels->64, 3x3) -> BN -> ReLU -> 5x SE-ResBlock(64)
  Policy: Conv(64->2, 1x1) -> BN -> ReLU -> FC(2*N*N, (N+1)*N*N) -> logits
  Value:  Conv(64->1, 1x1) -> BN -> ReLU -> FC(N*N, 64) -> ReLU -> FC(64, 3) -> WDL logits

Input planes (5*N+5 total, e.g. 30 for N=5):
  [0:N+1]        Card value presence (binary plane per value 0..N)
  [N+1:2N+2]     Current player's hand (one plane per value, count/(N+1) normalized)
  [2N+2:3N+3]    Opponent's hand (same)
  [3N+3]         Current player total score (min line score), normalized /100
  [3N+4:4N+4]    Current player per-line scores (line 0..N-1), normalized /100
  [4N+4]         Opponent total score, normalized /100
  [4N+5:5N+5]    Opponent per-line scores (line 0..N-1), normalized /100

Removed: empty plane (derivable from card planes), playable plane (derivable by
conv net), turn indicator (board transposed for Player B — encoding is
player-invariant; turn plane was a value-head collapse shortcut).
"""

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F


def screlu(x: torch.Tensor) -> torch.Tensor:
    """SCReLU: concatenate squared and linear clipped-relu branches, doubling channels."""
    c = x.clamp(0.0, 1.0)
    return torch.cat([c * c, c], dim=1)


def in_channels(n: int) -> int:
    """Number of input planes for an N×N board."""
    return 5 * n + 5


def num_card_values(n: int) -> int:
    """Number of distinct card values for an N×N board: 0..N inclusive."""
    return n + 1


def action_size(n: int) -> int:
    """Number of policy outputs: (N+1) card values × N² positions."""
    return (n + 1) * n * n


def encode_state(board: list[list[int | None]], hand_current: dict[int, int], hand_opponent: dict[int, int], current_player: int, board_size: int = 5) -> np.ndarray:
    """Encode a game state as a (in_channels(N), N, N) float32 tensor.

    Args:
        board: NxN grid, None for empty cells, int 0..N for card values.
        hand_current: {card_value: count} for the player to move.
        hand_opponent: {card_value: count} for the other player.
        current_player: 0 for Player A, 1 for Player B.
        board_size: N.

    Returns:
        np.ndarray of shape (in_channels(N), N, N).
    """
    n = board_size
    ch_hand_cur = n + 1
    ch_hand_opp = 2 * n + 2
    ch_score_cur = 3 * n + 3
    ch_score_opp = ch_score_cur + n + 1

    planes = np.zeros((in_channels(n), n, n), dtype=np.float32)

    # Card planes: for Player B, transpose board reads (row↔col) so the NN always
    # sees "my scoring dimension is columns", matching the Rust encoder.
    for r in range(n):
        for c in range(n):
            br, bc = (c, r) if current_player == 1 else (r, c)
            cell = board[br][bc]
            if cell is not None:
                planes[cell, r, c] = 1.0

    # hand planes: one per card value, count/(N+1) normalized, broadcast across spatial dims
    norm = float(n + 1)
    for v in range(num_card_values(n)):
        cnt_cur = hand_current.get(v, 0) / norm
        cnt_opp = hand_opponent.get(v, 0) / norm
        if cnt_cur > 0.0:
            planes[ch_hand_cur + v, :, :] = cnt_cur
        if cnt_opp > 0.0:
            planes[ch_hand_opp + v, :, :] = cnt_opp

    # Score planes — broadcast scalar across all N² spatial cells, normalized /100.
    # board.line(player, i): col i for A, row i for B. After the transpose above,
    # spatial column i in the NN's view matches scoring line i for each player.
    def line_score(line: list[int | None]) -> float:
        counts: dict[int, int] = {}
        for v in line:
            if v is not None:
                counts[v] = counts.get(v, 0) + 1
        s = 0
        for v, c in counts.items():
            s += v if c == 1 else (10 * v if c == 2 else 100)
        return float(s)

    # Current player's scoring lines: col i for A, row i for B
    for i in range(n):
        if current_player == 0:
            cur_line = [board[r][i] for r in range(n)]
        else:
            cur_line = [board[i][c] for c in range(n)]
        planes[ch_score_cur + 1 + i, :, :] = line_score(cur_line) / 100.0

    planes[ch_score_cur, :, :] = min(planes[ch_score_cur + 1 + i, 0, 0] for i in range(n))

    # Opponent's scoring lines: row i for A's opponent (B), col i for B's opponent (A)
    for i in range(n):
        if current_player == 0:
            opp_line = [board[i][c] for c in range(n)]
        else:
            opp_line = [board[r][i] for r in range(n)]
        planes[ch_score_opp + 1 + i, :, :] = line_score(opp_line) / 100.0

    planes[ch_score_opp, :, :] = min(planes[ch_score_opp + 1 + i, 0, 0] for i in range(n))

    return planes


class SEBlock(nn.Module):
    """Squeeze-and-Excitation block (Lc0-style with bias)."""

    def __init__(self, channels: int, se_channels: int):
        super().__init__()
        self.pool = nn.AdaptiveAvgPool2d(1)
        self.fc1 = nn.Linear(channels, se_channels)
        self.fc2 = nn.Linear(se_channels, 2 * channels)
        self.channels = channels

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        b, c, _, _ = x.shape
        squeezed = self.pool(x).view(b, c)
        excited = F.leaky_relu(self.fc1(squeezed))
        excited = self.fc2(excited)
        w, bias = excited.split(self.channels, dim=1)
        w = torch.sigmoid(w).view(b, c, 1, 1)
        bias = bias.view(b, c, 1, 1)
        return w * x + bias


class ResBlock(nn.Module):
    """Residual block with SE."""

    def __init__(self, channels: int, se_channels: int):
        super().__init__()
        self.conv1 = nn.Conv2d(channels, channels, 3, padding=1, bias=False)
        self.bn1 = nn.BatchNorm2d(channels)
        self.conv2 = nn.Conv2d(channels, channels, 3, padding=1, bias=False)
        self.bn2 = nn.BatchNorm2d(channels)
        self.se = SEBlock(channels, se_channels)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        residual = x
        out = F.leaky_relu(self.bn1(self.conv1(x)))
        out = self.bn2(self.conv2(out))
        out = self.se(out)
        return F.leaky_relu(out + residual)


class RobotMasterResNet(nn.Module):
    """AlphaZero-style dual-headed SE-ResNet for Robot Master.

    Policy output: (N+1) * N^2 logits (card_value * N*N + row * N + col).
    Value output: [B, 3] raw WDL logits (Win, Draw, Loss order).
    """

    def __init__(self, board_size: int = 5, num_blocks: int = 5, num_filters: int = 64):
        super().__init__()
        self.board_size = board_size
        self.num_filters = num_filters
        n2 = board_size * board_size

        # input projection
        self.conv_in = nn.Conv2d(in_channels(board_size), num_filters, 3, padding=1, bias=False)
        self.bn_in = nn.BatchNorm2d(num_filters)

        # residual tower
        se_channels = max(num_filters // 8, 4)
        self.blocks = nn.ModuleList([ResBlock(num_filters, se_channels) for _ in range(num_blocks)])

        # policy head
        self.policy_conv = nn.Conv2d(num_filters, 2, 1, bias=False)
        self.policy_bn = nn.BatchNorm2d(2)
        self.policy_fc = nn.Linear(4 * n2, action_size(board_size))  # 2x from SCReLU

        # value head
        self.value_conv = nn.Conv2d(num_filters, 1, 1, bias=False)
        self.value_bn = nn.BatchNorm2d(1)
        self.value_fc1 = nn.Linear(2 * n2, 64)  # 2x from SCReLU
        self.value_fc2 = nn.Linear(128, 3)  # 2x from SCReLU; outputs WDL logits

    def forward(self, x: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        # body
        out = F.leaky_relu(self.bn_in(self.conv_in(x)))
        for block in self.blocks:
            out = block(out)

        # policy head
        p = screlu(self.policy_bn(self.policy_conv(out)))
        p = p.flatten(1)
        p = self.policy_fc(p)

        # value head — raw WDL logits [B, 3]: (Win, Draw, Loss)
        v = screlu(self.value_bn(self.value_conv(out)))
        v = v.flatten(1)
        v = screlu(self.value_fc1(v))
        v = self.value_fc2(v)

        return p, v


if __name__ == "__main__":
    board_size = 5
    model = RobotMasterResNet(board_size=board_size)
    total_params = sum(p.numel() for p in model.parameters())
    print(f"Parameters: {total_params:,}")

    batch = torch.randn(4, in_channels(board_size), board_size, board_size)
    model.eval()
    with torch.no_grad():
        policy, value = model(batch)

    assert policy.shape == (4, action_size(board_size)), f"policy shape: {policy.shape}"
    assert value.shape == (4, 3), f"value shape: {value.shape}"
    wdl = value.softmax(dim=-1)
    v_scalar = wdl[:, 0] - wdl[:, 2]
    assert (v_scalar.abs() <= 1.0).all(), "value scalar out of [-1, 1]"
    print(f"Policy shape: {policy.shape}  (expected (4, {action_size(board_size)}))")
    print(f"Value shape:  {value.shape}  (expected (4, 3) WDL logits)")
    print(f"Value scalar range:  [{v_scalar.min():.4f}, {v_scalar.max():.4f}]")

    # encode_state smoke test
    board = [[None] * board_size for _ in range(board_size)]
    board[2][2] = 3  # center card
    hand_cur = {0: 2, 1: 2, 2: 2, 3: 1, 4: 2, 5: 3}
    hand_opp = {0: 1, 1: 3, 2: 2, 3: 2, 4: 2, 5: 2}
    planes = encode_state(board, hand_cur, hand_opp, current_player=0, board_size=board_size)
    n = board_size
    assert planes.shape == (in_channels(n), n, n), f"encode shape: {planes.shape}"
    assert planes[3, 2, 2] == 1.0, "card value 3 at center"
    assert planes[3, 0, 0] == 0.0, "card value 3 not at corner"
    # hand plane for value 0, cur: ch_hand_cur + 0 = n+1
    assert planes[n + 1, 0, 0] == hand_cur.get(0, 0) / (n + 1), "cur hand plane 0 normalized"
    # score planes: only board[2][2]=3 placed, so line score for col 2 = 3, others = 0
    ch_score_cur = 3 * n + 3
    assert planes[ch_score_cur + 1 + 2, 0, 0] == 3.0 / 100.0, "cur score plane for col 2"
    assert planes[ch_score_cur + 1 + 0, 0, 0] == 0.0, "cur score plane for col 0 (empty)"
    assert planes[ch_score_cur, 0, 0] == 0.0, "cur total score = min = 0"
    print("encode_state: OK")
    print("All checks passed.")
