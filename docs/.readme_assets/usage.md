## Usage

```sh
robot_master gui                          # main menu, pick players and board size
robot_master gui -a manual -b random     # skip straight to a game
```
To get a live eval bar, click `Settings -> Eval Mode`

Built-in players: `manual`, `random`, `greedy`, `sadist`, `rollout`. Board sizes: `5`, `7`, `9`, `11`. Append `|g<N>` to wrap any bot in Gumbel MCTS (`sadist|g200`). Named human players get Elo tracked automatically.

<!-- markdownlint-disable -->
<details>
<summary>
<h3>TUI / Arena</h3>
</summary>

```sh
# one-off match without touching saved ratings
robot_master arena --no-priors 'random,onnx:model_v15|g200' tourney swiss 20

# register a trained model, then run it in tournaments
robot_master arena players new 'onnx:model_v15|g200' --sizes 5
robot_master arena tourney swiss 10
```

`robot_master --help` covers the full player spec syntax, tourney modes, arena player management, and `--hide` (hidden-hand mode).

</details>
<!-- markdownlint-restore -->
