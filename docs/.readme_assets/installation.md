## Installation
### With Nix (recommended)
```sh
nix develop
```
This sets up Rust nightly, Python 3.12, maturin, cargo-leptos, native libraries (Vulkan, Wayland, X11, ALSA), and a Python virtualenv with all dependencies.

Then build the Rust binary:
```sh
cargo b -p robot_master
```
or simply
```sh
nix build
```

For AlphaZero training, also install the training deps:
```sh
uv_sync  # alias for: uv sync --prerelease=allow --no-install-project --dev
uv sync --group train
```

### Without Nix (but mb don't)
NB: not actually tested, - you're on your own here

#### Requirements
- Rust nightly (1.92+)
- Python >= 3.12
- System libraries: `alsa-lib`, `udev`, `vulkan-loader`, `libxkbcommon`, `wayland` (+ X11 libs if on X11)
- [`fzf`](https://github.com/junegunn/fzf) (optional, for player name selection in TUI)

#### Steps
```sh
# build the main binary
cargo b -p robot_master

# training dependencies (torch, onnx, tensorboard)
pip install torch numpy onnx onnxruntime tensorboard
```
