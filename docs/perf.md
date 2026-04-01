# Self-play Performance

## Setup

Hardware: (your GPU here)
Build: `cargo b --release -p robot_master_train`
Config: `--sims 16 --batch-size 128`

**Note on CUDA init cost:** loading the CUDA provider shared libraries takes ~0.1–6s depending on whether they are warm in the OS page cache. This cost is paid once per process invocation regardless of `--force-cpu` (the libs are compiled in). It is excluded from all game-time figures below; use wall-clock time from `time ./selfplay ...` to include it when comparing iteration overhead.

---

## 5×5 board — 1000 games, sims=16

Measured over 5 runs each (model_v59, release build):

| Mode | ms / game |
|---|---|
| CPU (`--force-cpu`, sequential rayon) | **10–11 ms** |
| GPU (default, batched) | 27–32 ms |

`--force-cpu` uses sequential rayon (one `evaluate` call per MCTS leaf, all threads independent) and is **~2.5–3× faster** than the GPU batched path at 5×5. Batching serialises what rayon does in parallel — pure coordination overhead with no GPU benefit when the model is this small.

---

## Larger board sizes

> Benchmarks pending — no trained models available yet for 7×7, 9×9, 11×11.
> Expected: GPU advantage grows with board size as model compute per sample scales as O(N²) while kernel-launch overhead stays constant.
> Rule of thumb: GPU default likely wins at 9×9 and above.

---

## --force-cpu guidance

`--force-cpu` skips CUDA and runs sequential rayon self-play (one NN call per leaf per thread). It is useful when:

- Board size is 5×5 or 7×7 (sequential rayon beats GPU batching here)
- A concurrent training job is using the GPU and you want to leave it free
- You are on a machine without a CUDA-capable GPU

At 9×9 and above, GPU batching is expected to be strictly faster. See above table (pending) for measured numbers.
