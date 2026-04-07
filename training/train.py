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

from model_resnet import RobotMasterResNet, action_size, in_channels
from model_transformer import RobotMasterTransformer


class SelfPlayDataset(Dataset):
    """Loads (state, policy_target, value_target) samples from binary files.

    File format per sample (written by Rust selfplay binary):
        state:  in_channels(N) * N * N float32 values (row-major)
        policy: (N+1) * N * N float32 values (visit count distribution, already normalized)
        value:  1 float32 (MCTS root mean from perspective of player to move, in [-1, 1])
    """

    def __init__(self, data_dir: str, board_size: int = 5, max_iters: int = 0):
        self.board_size = board_size
        n2 = board_size * board_size
        self.state_size = in_channels(board_size) * n2
        self.policy_size = action_size(board_size)
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
        state = np.array(floats[: self.state_size], dtype=np.float32).reshape(in_channels(n), n, n)
        policy = np.array(floats[self.state_size : self.state_size + self.policy_size], dtype=np.float32)
        value = np.float32(floats[-1])

        return torch.from_numpy(state), torch.from_numpy(policy), torch.tensor(value)


def alpha_zero_loss(
    policy_logits: torch.Tensor, value_pred: torch.Tensor, policy_target: torch.Tensor, value_target: torch.Tensor, value_weight: float = 0.25
) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
    """L = value_weight * MSE(v, z) + KL(pi || p).

    Policy loss: KL divergence matching tmp/minizero/train.py:133.
    KL divergence is correct for Gumbel AlphaZero — the improved policy target
    is not uniform, so cross-entropy and KL diverge in their gradients.

    Returns (total_loss, value_loss, policy_loss) for logging.
    """
    value_loss = F.mse_loss(value_pred, value_target)
    # KL(pi || p) = sum(pi * (log pi - log p)); PyTorch kl_div takes (log_q, p).
    policy_loss = F.kl_div(
        F.log_softmax(policy_logits, dim=1),
        policy_target,
        reduction='none',
    ).sum(dim=1).mean()
    return value_weight * value_loss + policy_loss, value_loss, policy_loss


def train(args: argparse.Namespace) -> None:
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"Device: {device}")

    if args.model == "transformer":
        model = RobotMasterTransformer(board_size=args.board_size)
    else:
        model = RobotMasterResNet(board_size=args.board_size)
    model.to(device)

    dataset = SelfPlayDataset(args.data_dir, board_size=args.board_size, max_iters=args.max_iters)
    print(f"Samples: {len(dataset)}")
    if len(dataset) == 0:
        print("No training data found.")
        return

    loader = DataLoader(dataset, batch_size=args.batch_size, shuffle=True, drop_last=True, num_workers=2, pin_memory=True)

    optimizer = torch.optim.SGD(model.parameters(), lr=args.lr, momentum=0.9, weight_decay=args.weight_decay)
    # Cosine schedule spans the full training run (steps_per_iter * total_iters), not just
    # one iteration. MiniZero trains with a single continuous schedule over 60k total steps.
    # We restore last_epoch so the scheduler resumes at the right position after each call.
    global_step_offset = 0
    if args.resume:
        ckpt = torch.load(args.resume, map_location="cpu", weights_only=True)
        ckpt_model_type = ckpt.get("model_type", "resnet")
        if ckpt_model_type != args.model:
            raise ValueError(f"Checkpoint model type '{ckpt_model_type}' does not match --model '{args.model}'")
        model.load_state_dict(ckpt["model"])
        optimizer.load_state_dict(ckpt["optimizer"])
        global_step_offset = ckpt.get("global_step", 0)
    # Cosine schedule spans the full training run, not just one iteration call.
    # last_epoch restores the scheduler's position so LR continues from where it left off.
    scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=args.total_steps, last_epoch=global_step_offset if global_step_offset > 0 else -1)

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
        loss, v_loss, p_loss = alpha_zero_loss(policy_logits, value_pred, policy_targets, value_targets, args.value_weight)

        optimizer.zero_grad()
        loss.backward()
        optimizer.step()
        scheduler.step()

        total_loss += loss.item()
        global_step = global_step_offset + step + 1
        if global_step % 100 == 0:
            writer.add_scalar("loss/total", loss.item(), global_step)
            writer.add_scalar("loss/value", v_loss.item(), global_step)
            writer.add_scalar("loss/policy", p_loss.item(), global_step)
            writer.add_scalar("lr", scheduler.get_last_lr()[0], global_step)

    avg = total_loss / max(args.steps, 1)
    print(f"Steps {args.steps}  loss={avg:.4f}")

    checkpoint = {
        "model": model.state_dict(),
        "optimizer": optimizer.state_dict(),
        "global_step": global_step_offset + args.steps,
        "model_type": args.model,
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
    parser.add_argument("--model", choices=["resnet", "transformer"], default="resnet", help="Model architecture to train")
    parser.add_argument("--batch-size", type=int, default=256)
    # MiniZero: steps proportional to games collected, ratio 1:10 (final.tex line 299).
    # Computed by train_cnn.rs as games // 10; passed explicitly — no default here.
    parser.add_argument("--steps", type=int, required=True)
    parser.add_argument("--total-steps", type=int, required=True, help="Total steps across all iterations, for cosine schedule T_max")
    # MiniZero (arxiv 2310.11305, table 2) uses lr=0.1 with SGD+momentum=0.9 for board games.
    # Original AlphaZero (1712.01815) also starts at 0.1. Our previous 0.02 was 5x too low.
    parser.add_argument("--lr", type=float, default=0.1)
    parser.add_argument("--weight-decay", type=float, default=1e-4)
    parser.add_argument("--resume", default=None, help="Resume from checkpoint")
    parser.add_argument("--max-iters", type=int, default=0, help="Cap replay buffer to this many most-recent iteration files (0 = no cap)")
    parser.add_argument("--value-weight", type=float, default=1.0, help="Weight for value loss: total = value_weight * MSE + KL. Set to ln(avg_legal_moves) for balanced gradients at init. MiniZero uses 0.25 (calibrated for Go ~200 legal moves). Default 1.0 = AlphaZero.")
    train(parser.parse_args())
