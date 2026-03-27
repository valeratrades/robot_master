# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).parent.parent))

try:
    from icecream import ic
except ImportError:  # Graceful fallback if IceCream isn't installed.
    ic = lambda *a: None if not a else (a[0] if len(a) == 1 else a)  # noqa

from dataclasses import dataclass

from partie_guidee.a_plateau import Grid
from partie_guidee.b_gestionCartes import emplacement_jouable
from partie_guidee.d_score import colonne_to_dico, score_ligne
from typeguard import typechecked


@dataclass(frozen=True, slots=True)
class Pos:
	L: int
	C: int


@typechecked
def choix_carte_greedy(plateau: Grid, dico_main: dict[int, int], dico_options: dict[str, int], joueuse_active: int) -> tuple[int, int, int]:
	"""La fonction choix_carte_greedy retourne un tuple (carte,posL,posC) maximisant le score_complet de la joueuse_active.

	# Assumptions
	We optimise each stack in isolation, taking the local maximum delta per move. No weight is given to
	which stack is likely to end up as the minimum (and thus the actual final score) - that prediction
	would require either look-ahead or a probabilistic model of the opponent's play.
	# Assumptions
	We optimise each stack in isolation, taking the local maximum delta per move. No weight is given to
	which stack is likely to end up as the minimum (and thus the actual final score) - that prediction
	would require either look-ahead or a probabilistic model of the opponent's play.
	"""

	hand = dico_main

	#TODO: switch everything to numparrays for speed

	# find all playable positions on the board.
	n = len(plateau)
	stacks: list[dict[int, int]] = list()
	for i in range(n):
		cards = colonne_to_dico(plateau, joueuse_active, i, dico_options)
		stacks.append(cards)

	available: list[Pos] = [
		Pos(L, C)
		for L in range(n)
		for C in range(n)
		if emplacement_jouable(plateau, L, C)
	]

	# map each row/col stack index to one representative Pos (both sides are score-equivalent).
	moves: list[Pos | None] = [None] * n
	for pos in available:
		stack_idx = pos.L if joueuse_active % 2 == 1 else pos.C
		if moves[stack_idx] is None:
			moves[stack_idx] = pos

	# for each stack with a valid Pos, score every card in hand by the delta it produces on that stack's score_ligne.
	maxC = dico_options["maxC"]

	scored_moves: list[tuple[tuple[int, int], ...] | None] = [None] * n
	for i, pos in enumerate(moves):
		if pos is None:
			continue
		stack = stacks[i]
		current_score = score_ligne(stack)
		entries: list[tuple[int, int]] = []
		for card_val in range(maxC + 1):
			if hand.get(card_val, 0) == 0:
				continue
			updated = dict(stack)
			updated[card_val] = updated.get(card_val, 0) + 1
			delta = score_ligne(updated) - current_score
			entries.append((card_val, delta))
		entries.sort(key=lambda x: (-x[1], x[0]))
		scored_moves[i] = tuple(entries)

	# return (card, posL, posC) with the highest delta; break ties by lowest card value.
	best_card: int | None = None
	best_pos: Pos | None = None
	best_delta: int | None = None

	for i, candidates in enumerate(scored_moves):
		if candidates is None or len(candidates) == 0:
			continue
		card_val, delta = candidates[0]
		if (
			best_delta is None
			or delta > best_delta
			or (delta == best_delta and card_val < best_card)
		):
			best_delta = delta
			best_card = card_val
			best_pos = moves[i]

	assert best_card is not None and best_pos is not None, "no valid move found"
	return (best_card, best_pos.L, best_pos.C)
