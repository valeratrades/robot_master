multi-player implementation of robot_master // in rust, because of course it is


## Reqs and `py_src/`
provisioned pdf with requirements: ./Sujet-RobotMaster-version-04-02.pdf

rough arch outline, functionality of each function, tests, desired behavior, - can be found in this pdf file

# Rules
1v1 on a 5x5 grid. Cards are numbered 0–5, with 6 copies each (36 total). Each player gets 12; a 25th card is placed at the center of the board.

**Turns**: players alternate placing a card from their hand onto an empty cell adjacent (no diagonals) to an occupied one.

**Scoring** (per line/column, once the grid is full):
| copies of a card | points |
|---|---|
| 1 | face value (0, 1, 2, 3, 4, or 5) |
| 2 | 10 × face value (0, 10, 20, 30, 40, or 50) |
| 3+ | 100 flat, regardless of face value |

**Winner**: Alice's score = her lowest-scoring column; Bob's score = his lowest-scoring row. Highest score wins.
