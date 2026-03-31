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

    def __init__(self, data_dir: str, board_size: int = 5):
        self.board_size = board_size
        n2 = board_size * board_size
        self.state_size = IN_CHANNELS * n2
        self.policy_size = NUM_CARD_VALUES * n2
        self.sample_floats = self.state_size + self.policy_size + 1
        self.sample_bytes = self.sample_floats * 4

        self.data = bytearray()
        data_path = Path(data_dir)
        for f in sorted(data_path.glob("*.bin")):
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

    dataset = SelfPlayDataset(args.data_dir, board_size=args.board_size)
    print(f"Samples: {len(dataset)}")
    if len(dataset) == 0:
        print("No training data found.")
        return

    loader = DataLoader(dataset, batch_size=args.batch_size, shuffle=True, drop_last=True, num_workers=2, pin_memory=True)

    optimizer = torch.optim.SGD(model.parameters(), lr=args.lr, momentum=0.9, weight_decay=args.weight_decay)
    scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=args.epochs * len(loader))

    writer = SummaryWriter(log_dir=str(Path(args.output_dir) / "tb"))
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    global_step = 0
    for epoch in range(args.epochs):
        model.train()
        epoch_loss = 0.0
        for states, policy_targets, value_targets in loader:
            states = states.to(device)
            policy_targets = policy_targets.to(device)
            value_targets = value_targets.to(device)

            policy_logits, value_pred = model(states)
            loss, v_loss, p_loss = alpha_zero_loss(policy_logits, value_pred, policy_targets, value_targets)

            optimizer.zero_grad()
            loss.backward()
            optimizer.step()
            scheduler.step()

            epoch_loss += loss.item()
            global_step += 1
            if global_step % 100 == 0:
                writer.add_scalar("loss/total", loss.item(), global_step)
                writer.add_scalar("loss/value", v_loss.item(), global_step)
                writer.add_scalar("loss/policy", p_loss.item(), global_step)
                writer.add_scalar("lr", scheduler.get_last_lr()[0], global_step)

        avg = epoch_loss / max(len(loader), 1)
        print(f"Epoch {epoch + 1}/{args.epochs}  loss={avg:.4f}")

        checkpoint = {
            "model": model.state_dict(),
            "optimizer": optimizer.state_dict(),
            "epoch": epoch + 1,
            "model_kwargs": {"board_size": args.board_size},
        }
        torch.save(checkpoint, output_dir / f"checkpoint_{epoch + 1:04d}.pt")

    writer.close()
    print(f"Training complete. Checkpoints in {output_dir}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Train Robot Master AlphaZero network")
    parser.add_argument("--data-dir", required=True, help="Directory with .bin self-play data")
    parser.add_argument("--output-dir", default="models", help="Where to save checkpoints")
    parser.add_argument("--board-size", type=int, default=5)
    parser.add_argument("--batch-size", type=int, default=256)
    parser.add_argument("--epochs", type=int, default=10)
    parser.add_argument("--lr", type=float, default=0.02)
    parser.add_argument("--weight-decay", type=float, default=1e-4)
    parser.add_argument("--resume", default=None, help="Resume from checkpoint")
    train(parser.parse_args())
