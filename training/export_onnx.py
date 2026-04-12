"""Export a trained PyTorch model to ONNX for Rust-side inference via `ort`.

Usage:
    python training/export_onnx.py --checkpoint models/checkpoint.pt --output models/model.onnx
    python training/export_onnx.py --checkpoint models/checkpoint.pt  # default output: same dir, .onnx ext
"""

import argparse
from pathlib import Path

import numpy as np
import torch
import onnxruntime as ort

from model_resnet import RobotMasterResNet, in_channels
from model_transformer import RobotMasterTransformer


def export(checkpoint_path: str, output_path: str, board_size: int = 5) -> None:
    checkpoint = torch.load(checkpoint_path, map_location="cpu", weights_only=True)

    model_kwargs = checkpoint.get("model_kwargs", {"board_size": board_size})
    model_type = checkpoint.get("model_type", "resnet")
    if model_type == "transformer":
        model = RobotMasterTransformer(**model_kwargs)
    else:
        model = RobotMasterResNet(**model_kwargs)
    model.load_state_dict(checkpoint["model"])
    model.eval()

    dummy = torch.randn(1, in_channels(model.board_size), model.board_size, model.board_size)

    # Wrap to drop the soft head — Rust inference only consumes policy + value.
    class ExportWrapper(torch.nn.Module):
        def __init__(self, inner: torch.nn.Module):
            super().__init__()
            self.inner = inner

        def forward(self, x: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
            policy, _policy_soft, value = self.inner(x)
            return policy, value

    export_model = ExportWrapper(model)

    torch.onnx.export(
        export_model,
        dummy,
        output_path,
        input_names=["state"],
        output_names=["policy", "value"],
        # value is [batch, 3] WDL logits; both dims are dynamic
        dynamic_axes={"state": {0: "batch"}, "policy": {0: "batch"}, "value": {0: "batch", 1: "wdl"}},
        opset_version=18,
        external_data=False,
        dynamo=False,
    )
    print(f"Exported to {output_path}")

    # validate roundtrip
    with torch.no_grad():
        pt_policy, pt_value = export_model(dummy)

    sess = ort.InferenceSession(output_path)
    onnx_policy, onnx_value = sess.run(None, {"state": dummy.numpy()})

    np.testing.assert_allclose(pt_policy.numpy(), onnx_policy, rtol=1e-3, atol=1e-4)
    # pt_value is [batch, 3] WDL logits; onnx_value shape should match
    np.testing.assert_allclose(pt_value.numpy(), onnx_value, rtol=1e-3, atol=1e-4)
    wdl = pt_value.softmax(dim=-1)
    v_scalar = wdl[:, 0] - wdl[:, 2]
    print(f"Sample value scalar: {v_scalar[0].item():.4f}  (win={wdl[0,0].item():.3f} draw={wdl[0,1].item():.3f} loss={wdl[0,2].item():.3f})")
    print("Roundtrip validation: OK")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Export Robot Master model to ONNX")
    parser.add_argument("--checkpoint", required=True, help="Path to .pt checkpoint")
    parser.add_argument("--output", default=None, help="Output .onnx path (default: derived from checkpoint)")
    parser.add_argument("--board-size", type=int, default=5)
    args = parser.parse_args()

    output = args.output or str(Path(args.checkpoint).with_suffix(".onnx"))
    export(args.checkpoint, output, args.board_size)
