from __future__ import annotations

from collections import Counter  # noqa: F401  # re-exported for test files

from a_plateau import Grid, plateau_to_string
from inline_snapshot import snapshot


def new_plateau_test() -> Grid:
    return [
        [None, None, 1, 1, 0],
        [None, 2, None, 3, None],
        [4, None, None, None, None],
        [None, 2, None, None, 0],
        [4, 4, 4, 0, 0],
    ]


plateau_test: Grid = new_plateau_test()


def new_small_plateau_test() -> Grid:
    return [[None, 1, 2], [3, None, None], [4, None, None]]


small_plateau_test: Grid = new_small_plateau_test()


def test_plateau_test_repr():
    assert plateau_to_string(plateau_test) == snapshot("""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |   | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |   |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""")


def test_small_plateau_test_repr():
    assert plateau_to_string(small_plateau_test) == snapshot("""\
---------------------
          0   1   2
---------------------
(0,_)   |   | 1 | 2 |
(1,_)   | 3 |   |   |
(2,_)   | 4 |   |   |
---------------------\
""")
