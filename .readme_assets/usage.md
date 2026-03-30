## Usage

The main binary is `robot_master`. It takes two players (`-a`, `-b`), an optional board size (`-s`), and a subcommand for the interface.

Players: `manual` (`m`), `random` (`r`), `greedy` (`g`), `sadist` (`s`). Unrecognized names prompt registration as a named manual player (with Elo tracking), or fall back to `fzf` selection.

Board sizes: `5` (default), `7`, `9`, `11`.

### TUI
```sh
robot_master tui                            # you vs random AI, 5x5
robot_master tui -a greedy -b sadist -s 7   # watch two AIs fight on 7x7
robot_master tui -a Alice -b Bob            # two named humans, Elo tracked
```
In manual mode, the TUI prompts for card, row, column each turn. Invalid moves get a warning and re-prompt.

### GUI
```sh
robot_master gui
robot_master gui -a manual -b greedy
```
Bevy app with a main menu where you can pick players and board size from dropdowns before starting. Elo ratings are shown next to player names.

### Python
For running the project as pure Python (e.g. for grading), the Rust binary must be compiled first (`cargo b -p robot_master`). The Python modules in `py_src/` shell out to it.

```sh
python -m py_src guided -m   # partie guidée, manual (both players)
python -m py_src guided -r   # partie guidée, random (both players)
python -m py_src naive -g    # IA mode, greedy vs greedy
python -m py_src naive -a    # IA mode, sadist vs sadist
```

### Elo
Player ratings persist across games in `$XDG_DATA_HOME/robot_master/ratings.json`. Every named player (manual or AI) accumulates an Elo score. End-of-game output shows rating changes.
