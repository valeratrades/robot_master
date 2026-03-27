# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Grid
from partie_guidee.b_gestionCartes import emplacement_jouable
from partie_guidee.d_score import colonne_to_dico, score_ligne
from typeguard import typechecked


# Obligatoire :
@typechecked
def score_complet_joueuse(plateau: Grid, joueuse_active: int, dico_options: dict[str, int]) -> list[int]:
	"""Retourne la liste des scores de chaque colone de la joueuse active.

	Triée du plus petit au plus grand score.
	"""
	n = len(plateau)
	scores = [score_ligne(colonne_to_dico(plateau, joueuse_active, i, dico_options)) for i in range(n)]
	scores.sort()
	return scores


# Facultatif :

@typechecked
def copie_plateau(plateau: Grid) -> Grid:
	"""Crée un nouveau plateau, copie de l'argument tel que modifier la sortie n'impacte pas l'entré."""
	# PERF: Grid is list[list[int | None]] - scalars only, so row slicing is sufficient and far
	# cheaper than deepcopy (no object graph traversal).
	return [row[:] for row in plateau]


@typechecked
def cases_jouables(plateau: Grid) -> list[tuple[int, int]]:
	"""La fonction cases_jouable prend en argument un plateau.

	Elle renvoie la liste des cases jouable (vide avec une voisine non vide)
	"""
	n = len(plateau)
	return [(i, j) for i in range(n) for j in range(n) if emplacement_jouable(plateau, i, j)]


@typechecked
def cartes_jouables(dico_main: dict[int, int]) -> list[int]:
	"""La fonction cases_jouable prend en argument un dico_main.

	Elle renvoie la liste des cartes jouable (présentes dans la main)
	"""
	return [c for c, count in dico_main.items() if count > 0]


@typechecked
def cartes_jouees(plateau: Grid, dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	"""Retourne un dictionnaire comptant les occurrences de chaque carte déjà posée sur le plateau."""
	maxC = dico_options["maxC"]
	dico: dict[int, int] = {v: 0 for v in range(maxC + 1)}
	for row in plateau:
		for cell in row:
			if cell is not None:
				dico[cell] += 1
	return dico


@typechecked
def cartes_restantes(plateau: Grid, dico_options: dict[str, int] = {"maxC": 5, "nbC": 6}) -> dict[int, int]:
	"""Calcul les cartes possibles restante (non déjà joué)."""
	maxC = dico_options["maxC"]
	nbC = dico_options["nbC"]
	jouees = cartes_jouees(plateau, {"maxC": maxC})
	return {v: nbC - jouees[v] for v in range(maxC + 1)}
