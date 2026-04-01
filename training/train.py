"""AlphaZero training loop for Robot Master.

Reads self-play data from training_data/, trains the network, saves checkpoints.

Usage:
    python training/train.py --data-dir training_data/ --output-dir models/
"""

import argparse
import struct
from pathlib import Path

import numpy as np
import torch
import torch.nn.functional as F
from torch.utils.data import DataLoader, Dataset
from torch.utils.tensorboard import SummaryWriter

from model_resnet import IN_CHANNELS, NUM_CARD_VALUES, RobotMasterResNet


class SelfPlayDataset(Dataset):
    """Loads (state, policy_target, value_target) samples from binary files.

    File format per sample (written by Rust selfplay binary):
        state:  33 * N * N float32 values (row-major)
        policy: 6 * N * N float32 values (visit count distribution, already normalized)
        value:  1 float32 (+1 or -1, game outcome from perspective of player to move)
    """

    def __init__(self, data_dir: str, board_size: int = 5, max_iters: int = 0):
        self.board_size = board_size
        n2 = board_size * board_size
        self.state_size = IN_CHANNELS * n2
        self.policy_size = NUM_CARD_VALUES * n2
        self.sample_floats = self.state_size + self.policy_size + 1
        self.sample_bytes = self.sample_floats * 4

        # One .bin file = one selfplay iteration. Sort newest-first, cap by iteration count.
        data_path = Path(data_dir)
        files = sorted(data_path.glob("*.bin"), key=lambda f: f.stat().st_mtime, reverse=True)
        if max_iters > 0:
            files = files[:max_iters]

        self.data = bytearray()
        for f in files:
            self.data.extend(f.read_bytes())

        total_bytes = len(self.data)
        if total_bytes % self.sample_bytes != 0:
            truncated = total_bytes - (total_bytes % self.sample_bytes)
            self.data = self.data[:truncated]

        self.num_samples = len(self.data) // self.sample_bytes

    def __len__(self) -> int:
        return self.num_samples

    def __getitem__(self, idx: int) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        offset = idx * self.sample_bytes
        floats = struct.unpack_from(f"<{self.sample_floats}f", self.data, offset)

        n = self.board_size
        state = np.array(floats[: self.state_size], dtype=np.float32).reshape(IN_CHANNELS, n, n)
        policy = np.array(floats[self.state_size : self.state_size + self.policy_size], dtype=np.float32)
        value = np.float32(floats[-1])

        return torch.from_numpy(state), torch.from_numpy(policy), torch.tensor(value)


def alpha_zero_loss(
    policy_logits: torch.Tensor, value_pred: torch.Tensor, policy_target: torch.Tensor, value_target: torch.Tensor
) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
    """L = MSE(v, z) + CrossEntropy(p, pi).

    Returns (total_loss, value_loss, policy_loss) for logging.
    """
    value_loss = F.mse_loss(value_pred, value_target)
    policy_loss = -(policy_target * F.log_softmax(policy_logits, dim=1)).sum(dim=1).mean()
    return value_loss + policy_loss, value_loss, policy_loss


def train(args: argparse.Namespace) -> None:
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"Device: {device}")

    model = RobotMasterResNet(board_size=args.board_size)
    if args.resume:
        ckpt = torch.load(args.resume, map_location="cpu", weights_only=True)
        model.load_state_dict(ckpt["model"])
    model.to(device)

    dataset = SelfPlayDataset(args.data_dir, board_size=args.board_size, max_iters=args.max_iters)
    print(f"Samples: {len(dataset)}")
    if len(dataset) == 0:
        print("No training data found.")
        return

    loader = DataLoader(dataset, batch_size=args.batch_size, shuffle=True, drop_last=True, num_workers=2, pin_memory=True)

    optimizer = torch.optim.SGD(model.parameters(), lr=args.lr, momentum=0.9, weight_decay=args.weight_decay)
    scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=args.steps)

    writer = SummaryWriter(log_dir=str(Path(args.output_dir) / "tb"))
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    # MiniZero: fixed gradient steps per iteration, proportional to games collected
    # (final.tex line 299: 200 steps per 2000 games). Cycle through the loader repeatedly.
    loader_iter = iter(loader)
    total_loss = 0.0
    for step in range(args.steps):
        try:
            states, policy_targets, value_targets = next(loader_iter)
        except StopIteration:
            loader_iter = iter(loader)
            states, policy_targets, value_targets = next(loader_iter)

        states = states.to(device)
        policy_targets = policy_targets.to(device)
        value_targets = value_targets.to(device)

        policy_logits, value_pred = model(states)
        loss, v_loss, p_loss = alpha_zero_loss(policy_logits, value_pred, policy_targets, value_targets)

        optimizer.zero_grad()
        loss.backward()
        optimizer.step()
        scheduler.step()

        total_loss += loss.item()
        if (step + 1) % 100 == 0:
            writer.add_scalar("loss/total", loss.item(), step)
            writer.add_scalar("loss/value", v_loss.item(), step)
            writer.add_scalar("loss/policy", p_loss.item(), step)
            writer.add_scalar("lr", scheduler.get_last_lr()[0], step)

    avg = total_loss / max(args.steps, 1)
    print(f"Steps {args.steps}  loss={avg:.4f}")

    checkpoint = {
        "model": model.state_dict(),
        "optimizer": optimizer.state_dict(),
        "steps": args.steps,
        "model_kwargs": {"board_size": args.board_size},
    }
    torch.save(checkpoint, output_dir / "checkpoint.pt")

    writer.close()
    print(f"Training complete. Checkpoints in {output_dir}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Train Robot Master AlphaZero network")
    parser.add_argument("--data-dir", required=True, help="Directory with .bin self-play data")
    parser.add_argument("--output-dir", default="models", help="Where to save checkpoints")
    parser.add_argument("--board-size", type=int, default=5)
    parser.add_argument("--batch-size", type=int, default=256)
    # MiniZero: steps proportional to games collected, ratio 1:10 (final.tex line 299).
    # Computed by train_cnn.rs as games // 10; passed explicitly — no default here.
    parser.add_argument("--steps", type=int, required=True)
    # MiniZero (arxiv 2310.11305, table 2) uses lr=0.1 with SGD+momentum=0.9 for board games.
    # Original AlphaZero (1712.01815) also starts at 0.1. Our previous 0.02 was 5x too low.
    parser.add_argument("--lr", type=float, default=0.1)
    parser.add_argument("--weight-decay", type=float, default=1e-4)
    parser.add_argument("--resume", default=None, help="Resume from checkpoint")
    parser.add_argument("--max-iters", type=int, default=0, help="Cap replay buffer to this many most-recent iteration files (0 = no cap)")
    train(parser.parse_args())
