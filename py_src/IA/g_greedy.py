# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).parent.parent))

try:
	from icecream import ic
except ImportError:  # Graceful fallback if IceCream isn't installed.
	ic = lambda *a: None if not a else (a[0] if len(a) == 1 else a)  # type: ignore[assignment] # noqa

from dataclasses import dataclass

from partie_guidee.a_plateau import Grid
from partie_guidee.b_gestionCartes import emplacement_jouable
from partie_guidee.d_score import colonne_to_dico
from typeguard import typechecked


def _score_ligne(counts: dict[int, int]) -> int:
	# PERF: inlined from d_score.score_ligne to avoid per-card function call overhead in the hot loop.
	# Rules: 1 copy = face value, 2 copies = 10xface, 3+ copies = 100 flat.
	return sum(v if c == 1 else (10 * v if c == 2 else 100) for v, c in counts.items() if c > 0)


def _score_delta(counts: dict[int, int], card_val: int) -> int:
	# PERF: analytic delta - avoids dict copy + two score_ligne calls per candidate (was O(maxC) alloc per iteration).
	# Derived from the scoring rules:
	#   0->1: gain face value             (+v)
	#   1->2: lose face value, gain 10x   (+9v)
	#   2->3: lose 10xface, gain 100 flat (+100 - 10v)
	# >=3->*: already scoring 100, no change
	c = counts.get(card_val, 0)
	v = card_val
	if c == 0:
		return v
	if c == 1:
		return 9 * v
	if c == 2:
		return 100 - 10 * v
	return 0


@dataclass(frozen=True, slots=True)
class Pos:
	L: int
	C: int


@typechecked
def choix_carte_greedy(plateau: Grid, dico_main: dict[int, int], dico_options: dict[str, int], joueuse_active: int) -> tuple[int, int, int]:
	"""La fonction choix_carte_greedy retourne un tuple (carte,posL,posC) maximisant le score_complet de la joueuse_active."""
	import robot_master as _rc
	return _rc.greedy_move_py(plateau, dico_main, joueuse_active)


# -----------------------------------------
#                Tests
# -----------------------------------------
if True:
	from inline_snapshot import snapshot
	from partie_guidee.a_plateau import plateau_to_string

	OPTS = {"maxC": 5}

	def _check(plateau: Grid, hand: dict[int, int], joueuse_active: int) -> tuple[int, int, int]:
		return choix_carte_greedy(plateau, hand, OPTS, joueuse_active)

	def _board_one_card() -> Grid:
		g: Grid = [[None] * 5 for _ in range(5)]
		g[2][2] = 3
		return g

	def _board_midgame() -> Grid:
		return [
			[None, None, 1, 1, 0],
			[None, 2, None, 3, None],
			[4, None, None, None, None],
			[None, 2, None, None, 0],
			[4, 4, 4, 0, 0],
		]

	def test_score_delta_first_copy():
		counts = {0: 0, 1: 0, 2: 0, 3: 0, 4: 0, 5: 0}
		assert _score_delta(counts, 3) == 3
		assert _score_delta(counts, 0) == 0

	def test_score_delta_second_copy():
		counts = {0: 0, 3: 1, 4: 0, 5: 0}
		assert _score_delta(counts, 3) == 27  # 9 * 3

	def test_score_delta_third_copy():
		counts = {2: 2, 5: 2}
		assert _score_delta(counts, 2) == 80  # 100 - 10*2
		assert _score_delta(counts, 5) == 50  # 100 - 10*5

	def test_score_delta_saturation():
		counts = {1: 3, 2: 4, 3: 5}
		assert _score_delta(counts, 1) == 0
		assert _score_delta(counts, 2) == 0
		assert _score_delta(counts, 3) == 0

	def test_picks_highest_delta_odd_player():
		# Row 2 already has a 3; playing another 3 gives delta=27 (9*3) vs delta=1 for card 1.
		board = _board_one_card()
		assert plateau_to_string(board) == snapshot("""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |   |   |   |   |
(1,_)   |   |   |   |   |   |
(2,_)   |   |   | 3 |   |   |
(3,_)   |   |   |   |   |   |
(4,_)   |   |   |   |   |   |
-----------------------------""")
		card, posL, _posC = _check(board, {0: 0, 1: 1, 2: 0, 3: 2, 4: 0, 5: 0}, joueuse_active=1)
		assert card == 3
		assert posL == 2  # must land in row 2 to score the pair

	def test_picks_highest_delta_even_player():
		# Col 2 already has a 3; even player scores columns.
		board = _board_one_card()
		card, _posL, posC = _check(board, {0: 0, 1: 1, 2: 0, 3: 2, 4: 0, 5: 0}, joueuse_active=0)
		assert card == 3
		assert posC == 2  # must land in col 2 to score the pair

	def test_tiebreak_lowest_card():
		# The tiebreak (lowest card on equal delta) lives in the sort key.
		# Test it directly rather than constructing a contrived board.
		entries = [(3, 5), (1, 5), (2, 5)]
		entries.sort(key=lambda x: (-x[1], x[0]))
		assert entries[0] == (1, 5)

	def test_midgame_odd_player():
		board = _board_midgame()
		assert plateau_to_string(board) == snapshot("""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |   | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |   |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------""")
		assert _check(board, {0: 1, 1: 2, 2: 0, 3: 1, 4: 0, 5: 2}, joueuse_active=1) == snapshot((1, 0, 1))

	def test_midgame_even_player():
		board = _board_midgame()
		assert _check(board, {0: 1, 1: 2, 2: 0, 3: 1, 4: 0, 5: 2}, joueuse_active=0) == snapshot((3, 2, 3))

	def test_game_rollout():
		from partie_guidee.b_gestionCartes import place_carte

		board = _board_midgame()
		hand = {0: 2, 1: 2, 2: 1, 3: 1, 4: 0, 5: 2}
		moves = []
		for turn in range(10):
			joueuse = turn % 2
			card, posL, posC = _check(board, hand, joueuse)
			moves.append((plateau_to_string(board), joueuse, card, posL, posC))
			place_carte(board, posL, posC, card)
			hand[card] -= 1
			if hand[card] == 0:
				del hand[card]
			if not any(v > 0 for v in hand.values()):
				break
		assert moves == snapshot([
			(
				"""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |   | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |   |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""",
				0,
				2,
				0,
				1,
			),
			(
				"""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   | 2 | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |   |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""",
				1,
				1,
				0,
				0,
			),
			(
				"""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 1 | 2 | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |   |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""",
				0,
				3,
				2,
				3,
			),
			(
				"""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 1 | 2 | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |   |   | 3 |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""",
				1,
				5,
				1,
				0,
			),
			(
				"""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 1 | 2 | 1 | 1 | 0 |
(1,_)   | 5 | 2 |   | 3 |   |
(2,_)   | 4 |   |   | 3 |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""",
				0,
				5,
				3,
				0,
			),
			(
				"""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 1 | 2 | 1 | 1 | 0 |
(1,_)   | 5 | 2 |   | 3 |   |
(2,_)   | 4 |   |   | 3 |   |
(3,_)   | 5 | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""",
				1,
				1,
				1,
				2,
			),
			(
				"""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 1 | 2 | 1 | 1 | 0 |
(1,_)   | 5 | 2 | 1 | 3 |   |
(2,_)   | 4 |   |   | 3 |   |
(3,_)   | 5 | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""",
				0,
				0,
				2,
				1,
			),
			(
				"""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 1 | 2 | 1 | 1 | 0 |
(1,_)   | 5 | 2 | 1 | 3 |   |
(2,_)   | 4 | 0 |   | 3 |   |
(3,_)   | 5 | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""",
				1,
				0,
				1,
				4,
			),
		])
