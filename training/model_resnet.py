"""Track A: Small SE-ResNet for AlphaZero.

Architecture (default, 5x5 board):
  Input:  33 x N x N
  Body:   Conv(33->64, 3x3) -> BN -> ReLU -> 5x SE-ResBlock(64)
  Policy: Conv(64->2, 1x1) -> BN -> ReLU -> FC(2*N*N, 6*N*N) -> logits
  Value:  Conv(64->1, 1x1) -> BN -> ReLU -> FC(N*N, 64) -> ReLU -> FC(64, 1) -> tanh

Input planes (33 total):
  [0:6]   Card value presence (binary plane per value 0-5)
  [6]     Empty cells
  [7]     Playable cells (adjacent to occupied & empty)
  [8:20]  Current player's hand (2 planes per value: >=1, >=2)
  [20:32] Opponent's hand (same)
  [32]    Current player indicator (1.0 = Player A)
"""

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

NUM_CARD_VALUES = 6  # 0..5
IN_CHANNELS = 33


def encode_state(board: list[list[int | None]], hand_current: dict[int, int], hand_opponent: dict[int, int], current_player: int, board_size: int = 5) -> np.ndarray:
    """Encode a game state as a (33, N, N) float32 tensor.

    Args:
        board: NxN grid, None for empty cells, int 0-5 for card values.
        hand_current: {card_value: count} for the player to move.
        hand_opponent: {card_value: count} for the other player.
        current_player: 0 for Player A, 1 for Player B.
        board_size: N.

    Returns:
        np.ndarray of shape (33, N, N).
    """
    n = board_size
    planes = np.zeros((IN_CHANNELS, n, n), dtype=np.float32)

    for r in range(n):
        for c in range(n):
            cell = board[r][c]
            if cell is None:
                planes[6, r, c] = 1.0  # empty plane
            else:
                planes[cell, r, c] = 1.0  # card value plane

    # playable: empty + has occupied neighbour
    for r in range(n):
        for c in range(n):
            if board[r][c] is not None:
                continue
            for dr, dc in [(-1, 0), (1, 0), (0, -1), (0, 1)]:
                nr, nc = r + dr, c + dc
                if 0 <= nr < n and 0 <= nc < n and board[nr][nc] is not None:
                    planes[7, r, c] = 1.0
                    break

    # hand planes: 2 per card value (>=1, >=2), broadcast across spatial dims
    for v in range(NUM_CARD_VALUES):
        cnt_cur = hand_current.get(v, 0)
        cnt_opp = hand_opponent.get(v, 0)
        if cnt_cur >= 1:
            planes[8 + v * 2, :, :] = 1.0
        if cnt_cur >= 2:
            planes[8 + v * 2 + 1, :, :] = 1.0
        if cnt_opp >= 1:
            planes[20 + v * 2, :, :] = 1.0
        if cnt_opp >= 2:
            planes[20 + v * 2 + 1, :, :] = 1.0

    # current player indicator
    if current_player == 0:
        planes[32, :, :] = 1.0

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
        excited = F.relu(self.fc1(squeezed))
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
        out = F.relu(self.bn1(self.conv1(x)))
        out = self.bn2(self.conv2(out))
        out = self.se(out)
        return F.relu(out + residual)


class RobotMasterResNet(nn.Module):
    """AlphaZero-style dual-headed SE-ResNet for Robot Master.

    Policy output: 6 * N^2 logits (card_value * N*N + row * N + col).
    Value output: scalar in [-1, 1].
    """

    def __init__(self, board_size: int = 5, in_channels: int = IN_CHANNELS, num_blocks: int = 5, num_filters: int = 64):
        super().__init__()
        self.board_size = board_size
        self.num_filters = num_filters
        n2 = board_size * board_size
        action_size = NUM_CARD_VALUES * n2

        # input projection
        self.conv_in = nn.Conv2d(in_channels, num_filters, 3, padding=1, bias=False)
        self.bn_in = nn.BatchNorm2d(num_filters)

        # residual tower
        se_channels = max(num_filters // 8, 4)
        self.blocks = nn.ModuleList([ResBlock(num_filters, se_channels) for _ in range(num_blocks)])

        # policy head
        self.policy_conv = nn.Conv2d(num_filters, 2, 1, bias=False)
        self.policy_bn = nn.BatchNorm2d(2)
        self.policy_fc = nn.Linear(2 * n2, action_size)

        # value head
        self.value_conv = nn.Conv2d(num_filters, 1, 1, bias=False)
        self.value_bn = nn.BatchNorm2d(1)
        self.value_fc1 = nn.Linear(n2, 64)
        self.value_fc2 = nn.Linear(64, 1)

    def forward(self, x: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        # body
        out = F.relu(self.bn_in(self.conv_in(x)))
        for block in self.blocks:
            out = block(out)

        # policy head
        p = F.relu(self.policy_bn(self.policy_conv(out)))
        p = p.flatten(1)
        p = self.policy_fc(p)

        # value head
        v = F.relu(self.value_bn(self.value_conv(out)))
        v = v.flatten(1)
        v = F.relu(self.value_fc1(v))
        v = torch.tanh(self.value_fc2(v))

        return p, v.squeeze(-1)


if __name__ == "__main__":
    board_size = 5
    model = RobotMasterResNet(board_size=board_size)
    total_params = sum(p.numel() for p in model.parameters())
    print(f"Parameters: {total_params:,}")

    batch = torch.randn(4, IN_CHANNELS, board_size, board_size)
    model.eval()
    with torch.no_grad():
        policy, value = model(batch)

    assert policy.shape == (4, NUM_CARD_VALUES * board_size ** 2), f"policy shape: {policy.shape}"
    assert value.shape == (4,), f"value shape: {value.shape}"
    assert (value.abs() <= 1.0).all(), "value out of [-1, 1]"
    print(f"Policy shape: {policy.shape}  (expected (4, {NUM_CARD_VALUES * board_size**2}))")
    print(f"Value shape:  {value.shape}  (expected (4,))")
    print(f"Value range:  [{value.min():.4f}, {value.max():.4f}]")

    # encode_state smoke test
    board = [[None] * board_size for _ in range(board_size)]
    board[2][2] = 3  # center card
    hand_cur = {0: 2, 1: 2, 2: 2, 3: 1, 4: 2, 5: 3}
    hand_opp = {0: 1, 1: 3, 2: 2, 3: 2, 4: 2, 5: 2}
    planes = encode_state(board, hand_cur, hand_opp, current_player=0, board_size=board_size)
    assert planes.shape == (IN_CHANNELS, board_size, board_size), f"encode shape: {planes.shape}"
    assert planes[3, 2, 2] == 1.0, "card value 3 at center"
    assert planes[6, 0, 0] == 1.0, "empty at (0,0)"
    assert planes[6, 2, 2] == 0.0, "not empty at center"
    assert planes[7, 2, 1] == 1.0, "playable adjacent to center"
    assert planes[7, 0, 0] == 0.0, "not playable at corner"
    assert planes[32, 0, 0] == 1.0, "player A indicator"
    print("encode_state: OK")
    print("All checks passed.")
