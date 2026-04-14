## Usage

The main binary is `robot_master`. It takes two players (`-a`, `-b`), an optional board size (`-s`), and a subcommand for the interface.

Players: `manual`, `random`, `greedy`, `sadist`, `rollout`. Search wrapping: append `|v<N>` (vanilla UCT-MCTS) or `|g<N>` (Gumbel) sims - `rollout|v800`, `rollout|g800`, `sadist|v200`. Unrecognized names prompt registration as a named manual player (with Elo tracking), or fall back to `fzf` selection.

Board sizes: `5`, `7`, `9`, `11`.

`--hide`: hide opponent's hand (information-hidden mode). At most one player may be manual when `--hide` is set.

### GUI
```sh
robot_master gui
robot_master gui -a manual -b greedy
robot_master gui --sound                     # enable music and sound effects
```
Bevy app with a main menu where you can pick players and board size from dropdowns before starting. Elo ratings are shown next to player names.

<!-- markdownlint-disable -->
<details>
<summary>
<h3>If you want TUI</h3>
</summary>

```sh
robot_master tui                              # you vs random AI, 5x5
robot_master tui -a greedy -b sadist -s 7    # watch two AIs fight on 7x7
robot_master tui -a Alice -b Bob             # two named humans, Elo tracked
robot_master tui --hide                      # hidden-hand mode
```
In manual mode, the TUI prompts for card, row, column each turn. Invalid moves get a warning and re-prompt.

</details>
<!-- markdownlint-restore -->

### Arena
Run tournaments between AI players. Ratings use Glicko-2.

```sh
robot_master arena tourney swiss 10              # all registered players, 10 Swiss brackets
robot_master arena tourney rating 200            # rating-based pairing, 200 rounds
robot_master arena tourney elimination 5         # single-elimination, 5 cycles
robot_master arena tourney round-robin 3         # every player vs every other, 3 sweeps
robot_master arena -s 'rollout,sadist' tourney swiss 10    # filter players by regex
robot_master arena tourney --json swiss 10       # output results as JSON to stdout
```

All tourney modes accept `-t <N>` / `--threads <N>`.

**Ephemeral tournaments (no ratings DB):**
```sh
# run a one-off match between specific specs without touching saved ratings
robot_master arena --no-priors 'rollout|v50,onnx:model_v15|g200' tourney swiss 20
```
`--no-priors` accepts a comma-separated list of player specs and bypasses the ratings database entirely. Mutually exclusive with `--select`.

**Managing players:**
```sh
robot_master arena players list                  # show all players and ratings
robot_master arena players new                   # register all default variants
robot_master arena players new rollout|v800      # register a specific variant
robot_master arena players reset-ratings         # reset all ratings to default
robot_master arena players nuke                  # remove players from DB entirely
```

**ONNX models in the arena** - after training, register a model then include it in tourneys:
```sh
# bare: runs policy head directly (greedy argmax, no search)
robot_master arena players new 'onnx:model_v15'

# with Gumbel search
robot_master arena players new 'onnx:model_v15|g200'

# constrain to specific board size and hide mode (required for onnx bots)
robot_master arena players new 'onnx:model_v15|g200' --sizes 5 --hide true

# then run against other players
robot_master arena -s 'onnx:model_v15,rollout$,sadist' tourney swiss 20
```

Player spec constraint suffixes (encoded in the ID, used for filtering):
- `|s5` or `|s5,7` - restrict to specific board size(s)
- `|hh` - hidden-hand mode only; `|hv` - visible-hand only

Models are looked up in `./models` by default. Override with `--models-dir`.
