# Architecture

## Code Map

### `robot_master_core` — Game Primitives

The single source of truth for game rules. Const-generic on board size `N`.

- `board.rs` — `Board<const N>`: placement, adjacency, line scoring.
- `cards.rs` — `Hand`, `CardValue`, deck creation.
- `game.rs` — `GameState<const N>`, `Move`, implements the `board_game::Board` trait.
- `scoring.rs` — scoring rules and `victoire()`.
- `python.rs` — thin PyO3 surface kept alive only to pass professor-distributed tests in `py_src/`.

**Architecture Invariant:** no knowledge of AI, ratings, or IO. Nothing in `robot_master_core` depends on any other workspace crate.

**Architecture Invariant:** `Board<const N>` is `Copy` with no heap allocation. It is a value type.

### `robot_master_arena` — Strategies & Match Runner

Everything about *who plays* and *how a match proceeds*.

- `algos/` — player taxonomy: `PlayerKind` dispatches to Random, Greedy, Sadist, Rollout, OnnxPlayer.
- `player.rs` — `Bot<const N>` trait: the single interface all strategies implement.
- `match_.rs` — `Match<const N>` runs a game move-by-move. `DynMatch` is a type-erased wrapper for Bevy ECS. `MatchResult` carries move history and triggers Glicko-2 on drop.
- `tournament.rs` — rating-based, Swiss, and elimination runners.
- `rating.rs` — Glicko-2 implementation.
- `db.rs` — `RatingDb` trait; JSON and Clickhouse backends.

**Architecture Invariant:** this crate does not load ONNX models. `OnnxPlayer` variant exists in the enum but `into_bot()` panics on it. Binary crates construct `NnEval` and inject it.

**Architecture Invariant:** `DynMatch` exists solely to bridge const-generic game logic into Bevy's `Any`-based ECS. Nothing outside `robot_master_game` should reach for it.

### `robot_master_train` — AlphaZero Pipeline

Gumbel AlphaZero selfplay and model evaluation.

- `gumbel.rs` — Sequential Halving guided by Gumbel noise. Returns improved policy π′. No Dirichlet noise.
- `mcts.rs` — `Tree`/`Node`/`Edge` plus the `Evaluator<N>` trait (policy + value). Implemented by `RolloutEval` and `NnEval`.
- `nn_eval.rs` — `NnEval`: loads `.onnx` with CUDA execution provider, batches inference, implements both `Evaluator<N>` and `Bot<N>`.
- `selfplay.rs` — `play_game()` → `Vec<Sample>` (state planes, policy target, value target). `play_games_batched()` runs N concurrent games sharing one NN evaluator.
- `encoding.rs` — game state → 33 input planes; action index ↔ `Move`.

**Architecture Invariant:** selfplay produces `.bin` sample files and exits. No connection to the arena or rating DB.

**Parallelism model — game-level batching, not tree-level threading.**
MiniZero uses multiple CPU threads searching the *same* tree in parallel, with virtual loss to prevent thread pile-up on a single path. We do the opposite: many independent games run concurrently (`play_games_batched`), each with its own tree (no sharing, no locking), and their NN calls are aggregated into one large `evaluate_batch` per loop iteration. This keeps the GPU saturated without any synchronization overhead. Virtual loss is therefore absent from our MCTS — it only makes sense when multiple threads compete on the same tree. For self-play training where sample throughput is the goal and GPU inference is the bottleneck, game-level batching is strictly better.

### `robot_master` — CLI Entry Point

Dispatches on subcommand and board size.

- `config.rs` — `Cli` + `Commands` (Tui / Gui / Arena), `LiveSettings`.
- `tui.rs` — terminal match runner, wraps bots with Gumbel if `sims.is_some()`.
- `arena.rs` — discovers `.onnx` models, resolves player filters (regex + fzf), drives tournament runners.
- `main.rs` — `size match (5/7/9/11) → run_sized::<N>()`.

**Architecture Invariant:** `robot_master` is the only crate that constructs `NnEval` + `GumbelBot<NnEval>` from a file path.

### `robot_master_game` — Bevy GUI

Standalone Bevy application: menu → match → result.

- `lib.rs` — `create_app(asset_dir)` (native) / `create_app()` (wasm). Three `AppState`s.
- `gameplay.rs` — match ECS logic, drives `DynMatch`.

**Architecture Invariant:** does not parse CLI args. All configuration enters via `InitialPlayers` resource injected before the app starts.

## Cross-Cutting Concerns

### Board Size Dispatch

Every binary-facing crate repeats `match size { 5 => …, 7 => …, 9 => …, 11 => … }` to monomorphize `Board<const N>`. Intentional: the compiler enforces exhaustiveness and eliminates the dispatch cost on hot paths.

### Bot Wrapping

Any `Bot<N>` can be wrapped in `GumbelBot<E: Evaluator<N>>` to get MCTS-guided play. The `sims: Option<u32>` field on `PlayerKind` controls this. Everything downstream sees only `Box<dyn Bot<N>>`.

### Glicko-2 Ratings

`MatchResult::commit()` (also called on `Drop`) updates ratings in-place. The rating DB is injected top-down from the binary — arena and TUI share the same `RatingDb` trait object.
