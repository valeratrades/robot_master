# Self-play Performance

## Setup

Hardware: (your GPU here)
Build: `cargo b --release -p robot_master_train`
Config: `--sims 16 --batch-size 128`

**Note on CUDA init cost:** loading the CUDA provider shared libraries takes ~0.1–6s depending on whether they are warm in the OS page cache. This cost is paid once per process invocation regardless of `--force-cpu` (the libs are compiled in). It is excluded from all game-time figures below; use wall-clock time from `time ./selfplay ...` to include it when comparing iteration overhead.

---

## 5×5 board — 1000 games, sims=16, batch=128

Measured over 5 runs each (model_v59, release build):

| Mode | Total time (range) | ms / game (range) |
|---|---|---|
| GPU (default) | 27–32 s | 27–32 ms |
| CPU (`--force-cpu`) | 27–29 s | 27–29 ms |

At 5×5, GPU and CPU are **essentially equal** — ranges fully overlap. The model is tiny (~600k params, input 33×5×5) and ORT's CPU backend saturates available cores with batch=128 just as fast as CUDA. Use `--force-cpu` here if you want to avoid GPU contention with a concurrent training job.

*Pre-batching baseline (old arch, sequential batch=1 per game):* ~14.6s for 200 games (~73 ms/game) → **~2.5× slower** than the current batched path.

---

## Larger board sizes

> Benchmarks pending — no trained models available yet for 7×7, 9×9, 11×11.
> Expected: GPU advantage grows with board size as model compute per sample scales as O(N²) while kernel-launch overhead stays constant.
> Rule of thumb: switch to GPU default at 9×9 and above.

---

## --force-cpu guidance

`--force-cpu` skips registering CUDA as the ORT execution provider. It does **not** avoid loading the CUDA provider libraries (those are compiled in and loaded at startup). The flag is useful when:

- A concurrent training job is using the GPU and you want to leave it free
- You are on a machine without a CUDA-capable GPU (ORT would fall back to CPU anyway, but this makes it explicit)
- Board size is 5×5 or 7×7 and you prefer deterministic CPU scheduling

At 9×9 and above, GPU is expected to be strictly faster. See above table (pending) for measured numbers.
