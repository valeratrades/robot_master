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

from model_resnet import IN_CHANNELS, RobotMasterResNet


def export(checkpoint_path: str, output_path: str, board_size: int = 5) -> None:
    checkpoint = torch.load(checkpoint_path, map_location="cpu", weights_only=True)

    model_kwargs = checkpoint.get("model_kwargs", {"board_size": board_size})
    model = RobotMasterResNet(**model_kwargs)
    model.load_state_dict(checkpoint["model"])
    model.eval()

    dummy = torch.randn(1, IN_CHANNELS, model.board_size, model.board_size)

    torch.onnx.export(
        model,
        dummy,
        output_path,
        input_names=["state"],
        output_names=["policy", "value"],
        dynamic_axes={"state": {0: "batch"}, "policy": {0: "batch"}, "value": {0: "batch"}},
        opset_version=17,
    )
    print(f"Exported to {output_path}")

    # validate roundtrip
    with torch.no_grad():
        pt_policy, pt_value = model(dummy)

    sess = ort.InferenceSession(output_path)
    onnx_policy, onnx_value = sess.run(None, {"state": dummy.numpy()})

    np.testing.assert_allclose(pt_policy.numpy(), onnx_policy, rtol=1e-4, atol=1e-5)
    np.testing.assert_allclose(pt_value.numpy(), onnx_value, rtol=1e-4, atol=1e-5)
    print("Roundtrip validation: OK")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Export Robot Master model to ONNX")
    parser.add_argument("--checkpoint", required=True, help="Path to .pt checkpoint")
    parser.add_argument("--output", default=None, help="Output .onnx path (default: derived from checkpoint)")
    parser.add_argument("--board-size", type=int, default=5)
    args = parser.parse_args()

    output = args.output or str(Path(args.checkpoint).with_suffix(".onnx"))
    export(args.checkpoint, output, args.board_size)
