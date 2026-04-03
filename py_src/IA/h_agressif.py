# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).parent.parent))

try:
	from icecream import ic
except ImportError:
	ic = lambda *a: None if not a else (a[0] if len(a) == 1 else a)  # noqa # type: ignore[assignment]

from f_fonctions_additionelles import cartes_restantes, copie_plateau
from partie_guidee.a_plateau import Grid
from partie_guidee.b_gestionCartes import emplacement_jouable, place_carte
from partie_guidee.d_score import colonne_to_dico, score_ligne
from typeguard import typechecked


def _is_better_move(score: int, card: int, posL: int, posC: int, best_score: int, best_move: tuple[int, int, int]) -> bool:
	"""True if (score, card, posL, posC) beats the current best by the canonical tiebreak ordering.

	Primary: lower score (less potential for opponent). Tiebreak: lower card, then lower posL, then lower posC.
	Centralising this ensures choix_carte_agressif and the naive reference impl in tests agree exactly.
	"""
	return (score, card, posL, posC) < (best_score, *best_move)


@typechecked
def cases_vide_ligne(dico_ligne: dict[int, int], taille: int) -> int:
	"""Retourne le nombre de case vide dans le ligne donnée."""
	return taille - sum(dico_ligne.values())


@typechecked
def complete_et_score(
	dico_ligne: dict[int, int],
	dico_cartes_restantes: dict[int, int],
	scores_possibles: list[int],
	dico_options: dict[str, int],
	card_index: int = 0,
) -> None:
	"""Fonction récursive qui calcule toutes les complétions possibles d'une ligne et accumule leurs scores.

	Si la ligne est complète, ajoute son score à scores_possibles.
	Sinon, parcourt toutes les cartes restantes (présentes en au moins 1 exemplaire dans
	dico_cartes_restantes) ; pour chaque carte, calcule récursivement toutes les complétions
	possibles en ajoutant cette carte à la ligne et en la retirant du dictionnaire des cartes restantes.

	# Impl
	Rather than placing one card per slot (which would generate n! permutations of the same
	multiset), we iterate over card *values* in order and decide how many of each to use before
	moving to the next value. This enumerates combinations, not permutations - each distinct
	multiset of counts is visited exactly once.
	"""
	taille = dico_options.get("taille", 5)
	vides = cases_vide_ligne(dico_ligne, taille)

	# base case: line is full, record its score and unwind.
	if vides == 0:
		scores_possibles.append(score_ligne(dico_ligne))
		return

	cards = list(dico_cartes_restantes.keys())

	# base case: no more card values to consider - fill remaining slots however we like,
	# but there are no cards left so this branch is impossible; just drop it.
	if card_index >= len(cards):
		return

	card = cards[card_index]
	available = dico_cartes_restantes[card]

	# try placing 0..min(available, vides) copies of this card, then move to the next value.
	# Each count is a separate branch; we add one copy, recurse, then undo before the next branch.
	# PERF: mutating in-place + undoing avoids allocating a new dict per branch.
	for n in range(min(available, vides) + 1):
		complete_et_score(dico_ligne, dico_cartes_restantes, scores_possibles, dico_options, card_index + 1)
		if n < min(available, vides):
			dico_ligne[card] += 1
			dico_cartes_restantes[card] -= 1

	# undo all copies placed across the loop iterations
	placed = min(available, vides)
	dico_ligne[card] -= placed
	dico_cartes_restantes[card] += placed


@typechecked
def tous_les_scores_possibles(
	dico_ligne: dict[int, int],
	dico_cartes_restantes: dict[int, int],
	dico_options: dict[str, int],
) -> list[int]:
	"""Calcule toutes les complétions possibles d'une ligne et retourne la liste de tous leurs scores.

	Prend en argument un dico_ligne, un dictionnaire des cartes restantes (si tous les 5 sont déjà
	joués, aucun 5 ne peut compléter la ligne étudiée), et le dictionnaire des options.
	Délègue le travail récursif à complete_et_score.
	"""
	scores: list[int] = []
	complete_et_score(dict(dico_ligne), dict(dico_cartes_restantes), scores, dico_options)
	return scores


@typechecked
def score_max_potentiel_complet_joueuse(
	plateau: Grid,
	joueuse_active: int,
	dico_options: dict[str, int],
) -> int:
	"""Retourne le score complet maximal que peut atteindre une joueuse.

	Pour chaque ligne (ou colonne) de la joueuse, calcule le score de la meilleure complétion possible
	via tous_les_scores_possibles, et retourne le maximum sur toutes les lignes.
	"""
	n = plateau.__len__()
	restantes = cartes_restantes(plateau, dico_options)
	best = 0
	for i in range(n):
		ligne = colonne_to_dico(plateau, joueuse_active, i, dico_options)
		scores = tous_les_scores_possibles(ligne, restantes, dico_options)
		if scores:
			best = max(best, max(scores))
	return best


@typechecked
def choix_carte_agressif(
	plateau: Grid,
	dico_main: dict[int, int],
	dico_options: dict[str, int],
	joueuse_active: int,
) -> tuple[int, int, int]:
	"""Retourne le coup (carte, posL, posC) qui minimise le score maximal potentiel de l'adversaire."""
	import robot_master as _rc

	return _rc.sadist_move_py(plateau, dico_main, joueuse_active)


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

if True:
	import copy

	from inline_snapshot import snapshot
	from partie_guidee.a_plateau import display_diff, plateau_to_string

	OPTS = {"maxC": 5, "nbC": 6, "taille": 5}

	def _check(plateau: Grid, hand: dict[int, int], joueuse_active: int) -> tuple[int, int, int]:
		return choix_carte_agressif(plateau, hand, OPTS, joueuse_active)

	def _board_midgame() -> Grid:
		return [
			[None, None, 1, 1, 0],
			[None, 2, None, 3, None],
			[4, None, None, None, None],
			[None, 2, None, None, 0],
			[4, 4, 4, 0, 0],
		]

	def test_naive_vs_optimised_same_decisions_and_faster():
		from f_fonctions_additionelles import cartes_restantes as _cartes_restantes
		from partie_guidee.b_gestionCartes import place_carte

		OPTS9 = {"maxC": 5, "nbC": 6, "taille": 5}

		def tous_les_scores_possibles_naive(
			dico_ligne: dict[int, int],
			dico_cartes_restantes: dict[int, int],
			dico_options: dict[str, int],
		) -> list[int]:
			def rec(ligne: dict[int, int], restantes: dict[int, int]) -> None:
				if cases_vide_ligne(ligne, dico_options.get("taille", 5)) == 0:
					scores.append(score_ligne(ligne))
					return
				for card, count in restantes.items():
					if count == 0:
						continue
					ligne[card] = ligne.get(card, 0) + 1
					restantes[card] -= 1
					rec(ligne, restantes)
					ligne[card] -= 1
					restantes[card] += 1

			scores: list[int] = []
			rec(dict(dico_ligne), dict(dico_cartes_restantes))
			return scores

		def choix_naive(plateau: Grid, hand: dict[int, int], joueuse: int) -> tuple[int, int, int]:
			adversaire = 1 - joueuse
			best_move: tuple[int, int, int] | None = None
			best_score: int | None = None
			n = len(plateau)
			for posL in range(n):
				for posC in range(n):
					if not emplacement_jouable(plateau, posL, posC):
						continue
					for card in sorted(hand):
						if hand[card] == 0:
							continue
						p = copie_plateau(plateau)
						place_carte(p, posL, posC, card)
						restantes = _cartes_restantes(p, OPTS9)
						score = 0
						for i in range(n):
							ligne = colonne_to_dico(p, adversaire, i, OPTS9)
							s = tous_les_scores_possibles_naive(ligne, restantes, OPTS9)
							if s:
								score = max(score, max(s))
						if best_move is None or _is_better_move(score, card, posL, posC, best_score, best_move):  # type: ignore[arg-type]
							best_score = score
							best_move = (card, posL, posC)
			assert best_move is not None
			return best_move

		naive_leaves = 0
		opt_leaves = 0

		def tous_les_scores_possibles_naive_counting(ligne, restantes, opts):
			nonlocal naive_leaves
			result = tous_les_scores_possibles_naive(ligne, restantes, opts)
			naive_leaves += len(result)
			return result

		def tous_les_scores_possibles_counting(ligne, restantes, opts):
			nonlocal opt_leaves
			result = tous_les_scores_possibles(ligne, restantes, opts)
			opt_leaves += len(result)
			return result

		def choix_naive_counting(plateau: Grid, hand: dict[int, int], joueuse: int) -> tuple[int, int, int]:
			adversaire = 1 - joueuse
			best_move: tuple[int, int, int] | None = None
			best_score: int | None = None
			n = len(plateau)
			for posL in range(n):
				for posC in range(n):
					if not emplacement_jouable(plateau, posL, posC):
						continue
					for card in sorted(hand):
						if hand[card] == 0:
							continue
						p = copie_plateau(plateau)
						place_carte(p, posL, posC, card)
						restantes = _cartes_restantes(p, OPTS9)
						score = 0
						for i in range(n):
							ligne = colonne_to_dico(p, adversaire, i, OPTS9)
							s = tous_les_scores_possibles_naive_counting(ligne, restantes, OPTS9)
							if s:
								score = max(score, max(s))
						if best_move is None or _is_better_move(score, card, posL, posC, best_score, best_move):  # type: ignore[arg-type]
							best_score = score
							best_move = (card, posL, posC)
			assert best_move is not None
			return best_move

		def choix_opt_counting(plateau: Grid, hand: dict[int, int], joueuse: int) -> tuple[int, int, int]:
			adversaire = 1 - joueuse
			best_move: tuple[int, int, int] | None = None
			best_score: int | None = None
			n = len(plateau)
			for posL in range(n):
				for posC in range(n):
					if not emplacement_jouable(plateau, posL, posC):
						continue
					for card in sorted(hand):
						if hand[card] == 0:
							continue
						p = copie_plateau(plateau)
						place_carte(p, posL, posC, card)
						restantes = _cartes_restantes(p, OPTS9)
						score = 0
						for i in range(n):
							ligne = colonne_to_dico(p, adversaire, i, OPTS9)
							s = tous_les_scores_possibles_counting(ligne, restantes, OPTS9)
							if s:
								score = max(score, max(s))
						if best_move is None or _is_better_move(score, card, posL, posC, best_score, best_move):  # type: ignore[arg-type]
							best_score = score
							best_move = (card, posL, posC)
			assert best_move is not None
			return best_move

		board = _board_midgame()
		hand = {0: 2, 1: 2, 2: 1, 3: 1, 4: 0, 5: 2}

		for turn in range(5):
			joueuse = turn % 2
			best_naive = choix_naive_counting(board, hand, joueuse)
			best_opt = choix_opt_counting(board, hand, joueuse)

			assert best_opt == best_naive, f"turn {turn}: opt={best_opt} naive={best_naive}"

			card, posL, posC = best_opt
			place_carte(board, posL, posC, card)
			hand[card] -= 1
			if hand[card] == 0:
				del hand[card]
			if not any(v > 0 for v in hand.values()):
				break

		assert snapshot({"naive_leaves": 102619, "opt_leaves": 23369}) == {"naive_leaves": naive_leaves, "opt_leaves": opt_leaves}
		# NB: speedup from combination vs permutation enumeration only materialises with many
		# empty slots (early game / larger boards). On this late-game 5x5 board the savings are
		# negligible - the correctness check above is what matters here.
		# // and it's python, so can't just make it bigger without risking dying of old age while waiting

	def test_tous_les_scores_possibles_no_duplicates():
		# 2 empty slots, 2 copies of card 1 and 2 copies of card 2 available.
		# Distinct multiset completions: {1:2}, {1:1,2:1}, {2:2} -> scores 10, 3, 20.
		ligne = {0: 0, 1: 0, 2: 0}
		restantes = {0: 0, 1: 2, 2: 2}
		scores = tous_les_scores_possibles(ligne, restantes, {"maxC": 2, "taille": 2})
		assert sorted(scores) == snapshot([3, 10, 20])
		assert len(scores) == len(set(scores))  # no duplicates

	def test_tous_les_scores_possibles_already_complete():
		# Line already full - only one completion (itself), remaining cards irrelevant.
		ligne = {0: 0, 1: 2, 2: 3}
		restantes = {0: 1, 1: 1, 2: 1}
		scores = tous_les_scores_possibles(ligne, restantes, {"maxC": 2, "taille": 5})
		assert scores == snapshot([110])

	def test_agressif_midgame():
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
-----------------------------\
""")
		card, posL, posC = _check(board, {0: 1, 1: 2, 2: 0, 3: 1, 4: 0, 5: 2}, joueuse_active=1)
		after = copy.deepcopy(board)
		after[posL][posC] = card
		assert display_diff(after, board) == snapshot("""\
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |+0 | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |   |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""")
		assert (card, posL, posC) == snapshot((0, 0, 1))

	def test_agressif_game_rollout():
		from partie_guidee.b_gestionCartes import place_carte

		board = _board_midgame()
		hand = {0: 2, 1: 2, 2: 1, 3: 1, 4: 0, 5: 2}
		moves = []
		for turn in range(10):
			joueuse = turn % 2
			card, posL, posC = _check(board, hand, joueuse)
			prev = copy.deepcopy(board)
			place_carte(board, posL, posC, card)
			moves.append(f"turn={joueuse}\n{display_diff(board, prev)}")
			hand[card] -= 1
			if hand[card] == 0:
				del hand[card]
			if not any(v > 0 for v in hand.values()):
				break
		assert "\n---\n".join(moves) == snapshot("""\
turn=0
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |   | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |+0 |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------
---
turn=1
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |+0 | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 | 0 |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------
---
turn=0
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   | 0 | 1 | 1 | 0 |
(1,_)   |+1 | 2 |   | 3 |   |
(2,_)   | 4 | 0 |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------
---
turn=1
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   | 0 | 1 | 1 | 0 |
(1,_)   | 1 | 2 |   | 3 |+1 |
(2,_)   | 4 | 0 |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------
---
turn=0
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |+2 | 0 | 1 | 1 | 0 |
(1,_)   | 1 | 2 |   | 3 | 1 |
(2,_)   | 4 | 0 |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------
---
turn=1
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 2 | 0 | 1 | 1 | 0 |
(1,_)   | 1 | 2 |+3 | 3 | 1 |
(2,_)   | 4 | 0 |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------
---
turn=0
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 2 | 0 | 1 | 1 | 0 |
(1,_)   | 1 | 2 | 3 | 3 | 1 |
(2,_)   | 4 | 0 |+5 |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------
---
turn=1
-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   | 2 | 0 | 1 | 1 | 0 |
(1,_)   | 1 | 2 | 3 | 3 | 1 |
(2,_)   | 4 | 0 | 5 |   |+5 |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------\
""")
