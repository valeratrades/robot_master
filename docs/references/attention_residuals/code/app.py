"""
Interactive Attention Residuals visualization with Gradio.

Launch:
    python app.py --model_path output/scratch-block-d512-L12-20k/final --mode block
"""

import argparse
import sys
import os

import torch
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

from modeling_attnres import Qwen3AttnResForCausalLM, Qwen3AttnResConfig, block_attn_res
from transformers import AutoTokenizer


def compute_softmax_weights(blocks, partial_block, proj, norm, recency_bias):
    V = torch.stack(blocks + [partial_block], dim=0)
    K = norm(V)
    query = proj.weight.view(-1)
    logits = torch.einsum("d, n b t d -> n b t", query, K)
    logits[-1] = logits[-1] + recency_bias
    weights = logits.softmax(dim=0)
    return weights.float()


def extract_weights(model, input_ids):
    """Run forward pass and capture per-token softmax weights."""
    import modeling_attnres as attnres_mod
    captured = {"attn": {}, "mlp": {}}

    original_forwards = {}
    for layer_idx, layer in enumerate(model.model.layers):
        original_forwards[layer_idx] = layer.forward

        def make_patched_forward(orig_forward, lyr, idx):
            def patched_forward(blocks, partial_block, **kwargs):
                # Attention sublayer
                w_attn = compute_softmax_weights(
                    blocks, partial_block,
                    lyr.attn_res_proj, lyr.attn_res_norm, lyr.attn_res_bias)
                captured["attn"][idx] = {
                    "weights_mean": w_attn.mean(dim=(1, 2)).detach().cpu().numpy(),
                    "weights_all": w_attn.detach().cpu().numpy(),  # (N+1, B, T)
                }

                h = attnres_mod.block_attn_res(
                    blocks, partial_block,
                    lyr.attn_res_proj, lyr.attn_res_norm, lyr.attn_res_bias)
                attn_out, _ = lyr.self_attn(
                    hidden_states=lyr.input_layernorm(h),
                    attention_mask=kwargs.get("attention_mask"),
                    position_ids=kwargs.get("position_ids"),
                    past_key_values=kwargs.get("past_key_values"),
                    use_cache=kwargs.get("use_cache", False),
                    cache_position=kwargs.get("cache_position"),
                    position_embeddings=kwargs.get("position_embeddings"),
                )
                post_attn_partial = partial_block + attn_out

                if lyr.attnres_mode == "full":
                    blocks_mlp = blocks + [post_attn_partial]
                else:
                    blocks_mlp = blocks

                # MLP sublayer
                w_mlp = compute_softmax_weights(
                    blocks_mlp, post_attn_partial,
                    lyr.mlp_res_proj, lyr.mlp_res_norm, lyr.mlp_res_bias)
                captured["mlp"][idx] = {
                    "weights_mean": w_mlp.mean(dim=(1, 2)).detach().cpu().numpy(),
                    "weights_all": w_mlp.detach().cpu().numpy(),
                }

                h = attnres_mod.block_attn_res(
                    blocks_mlp, post_attn_partial,
                    lyr.mlp_res_proj, lyr.mlp_res_norm, lyr.mlp_res_bias)
                mlp_out = lyr.mlp(lyr.post_attention_layernorm(h))
                final_partial = post_attn_partial + mlp_out

                if lyr.attnres_mode == "full" or lyr.is_block_boundary:
                    new_blocks = blocks_mlp + [final_partial]
                else:
                    new_blocks = blocks_mlp
                return new_blocks, final_partial
            return patched_forward

        layer.forward = make_patched_forward(original_forwards[layer_idx], layer, layer_idx)

    try:
        with torch.no_grad():
            model(input_ids)
    finally:
        for layer_idx, layer in enumerate(model.model.layers):
            layer.forward = original_forwards[layer_idx]

    return captured


def plot_layer_deps(captured, num_layers, title=""):
    """Plot the Kimi-style layer dependency heatmap."""
    num_sublayers = num_layers * 2
    max_sources = 0
    for sub in ["attn", "mlp"]:
        for d in captured[sub].values():
            max_sources = max(max_sources, len(d["weights_mean"]))
    total_cols = max_sources

    matrix = np.full((num_sublayers, total_cols), np.nan)
    for layer_idx in range(num_layers):
        if layer_idx in captured["attn"]:
            w = captured["attn"][layer_idx]["weights_mean"]
            matrix[layer_idx * 2, :len(w)] = w
        if layer_idx in captured["mlp"]:
            w = captured["mlp"][layer_idx]["weights_mean"]
            matrix[layer_idx * 2 + 1, :len(w)] = w

    is_full = total_cols > 20
    fig_w = max(10, total_cols * 0.4 + 3) if is_full else 10
    fig, ax = plt.subplots(figsize=(fig_w, 8))
    cmap = plt.cm.viridis.copy()
    cmap.set_bad(color="#1a1a2e")
    masked = np.ma.masked_invalid(matrix)
    im = ax.imshow(masked, aspect="auto", cmap=cmap, vmin=0, vmax=0.7,
                   interpolation="nearest")

    y_labels = []
    y_colors = []
    for i in range(num_layers):
        y_labels.extend([f"Attn {i}", f"MLP {i}"])
        y_colors.extend(["#4CAF50", "#FF9800"])
    ax.set_yticks(range(num_sublayers))
    ax.set_yticklabels(y_labels, fontsize=6)
    for i, tick in enumerate(ax.get_yticklabels()):
        tick.set_color(y_colors[i])
        tick.set_fontweight("bold")

    ax.set_xlabel("Source", fontsize=10)
    ax.set_ylabel("Sublayer", fontsize=10)
    ax.set_title(title or "Attention Residuals: Layer Dependencies", fontsize=12)
    plt.colorbar(im, ax=ax, shrink=0.8, label="Weight")
    plt.tight_layout()
    return fig


def plot_token_weights(captured, tokens, layer_idx, sublayer, num_layers):
    """Plot per-token attention weights for a specific sublayer."""
    data = captured[sublayer].get(layer_idx)
    if data is None:
        fig, ax = plt.subplots()
        ax.text(0.5, 0.5, "No data", ha="center", va="center")
        return fig

    # weights_all shape: (N+1, 1, T) — squeeze batch dim
    w = data["weights_all"][:, 0, :]  # (N+1, T)
    n_sources, n_tokens = w.shape
    n_tokens = min(n_tokens, len(tokens))

    fig, ax = plt.subplots(figsize=(max(8, n_tokens * 0.5), max(4, n_sources * 0.4)))
    cmap = plt.cm.viridis.copy()
    im = ax.imshow(w[:, :n_tokens], aspect="auto", cmap=cmap, vmin=0, vmax=0.7)
    ax.set_xticks(range(n_tokens))
    ax.set_xticklabels(tokens[:n_tokens], rotation=45, ha="right", fontsize=7)
    ax.set_ylabel("Source")
    ax.set_title(f"Layer {layer_idx} {sublayer.upper()} — Per-Token Weights", fontsize=11)
    plt.colorbar(im, ax=ax, shrink=0.8)
    plt.tight_layout()
    return fig


def create_app(model, tokenizer, num_layers):
    import gradio as gr

    def run_visualization(text, view_type, layer_idx, sublayer):
        input_ids = tokenizer(text, return_tensors="pt")["input_ids"].to(
            next(model.parameters()).device)
        tokens = tokenizer.convert_ids_to_tokens(input_ids[0])
        captured = extract_weights(model, input_ids)

        if view_type == "Layer Dependencies (Heatmap)":
            fig = plot_layer_deps(captured, num_layers, title=f"AttnRes Weights — \"{text[:50]}...\"")
        else:
            fig = plot_token_weights(captured, tokens, int(layer_idx),
                                     sublayer.lower(), num_layers)
        return fig

    with gr.Blocks(title="Attention Residuals Visualizer") as app:
        gr.Markdown("# Attention Residuals: Interactive Visualization")
        gr.Markdown("Explore how layers selectively attend to earlier representations.")

        with gr.Row():
            text_input = gr.Textbox(
                value="The theory of general relativity describes gravity as the curvature of spacetime.",
                label="Input Text", lines=2)

        with gr.Row():
            view_type = gr.Radio(
                ["Layer Dependencies (Heatmap)", "Per-Token Weights"],
                value="Layer Dependencies (Heatmap)", label="View")
            layer_slider = gr.Slider(0, num_layers - 1, value=0, step=1, label="Layer")
            sublayer_radio = gr.Radio(["Attn", "MLP"], value="Attn", label="Sublayer")

        btn = gr.Button("Visualize", variant="primary")
        output = gr.Plot(label="Visualization")

        btn.click(fn=run_visualization,
                  inputs=[text_input, view_type, layer_slider, sublayer_radio],
                  outputs=output)

    return app


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model_path", required=True)
    parser.add_argument("--mode", required=True, choices=["block", "full"])
    parser.add_argument("--device", default="cuda:0")
    parser.add_argument("--share", action="store_true")
    args = parser.parse_args()

    print(f"Loading {args.mode} model from {args.model_path}...")
    model = Qwen3AttnResForCausalLM.from_pretrained(
        args.model_path, torch_dtype=torch.bfloat16, device_map={"": args.device})
    model.eval()
    tokenizer = AutoTokenizer.from_pretrained("Qwen/Qwen3-0.6B")
    num_layers = model.config.num_hidden_layers

    print("Launching Gradio app...")
    app = create_app(model, tokenizer, num_layers)
    app.launch(share=args.share, server_name="0.0.0.0")


if __name__ == "__main__":
    main()
