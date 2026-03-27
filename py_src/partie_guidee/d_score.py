# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Grid
from typeguard import typechecked


@typechecked
def init_dico_cartes(dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	"""La fonction init_dico_cartes crée un dictionnaire dont les clés sont des int correspondants aux valeurs possibles des cartes.

	Les valeurs sont initialisées à 0. maxC est une variables optionnelle donnant la valeur maximal d'une carte.
	"""
	maxC = dico_options["maxC"]
	return {v: 0 for v in range(maxC + 1)}


@typechecked
def colonne_to_dico(plateau: Grid, joueuse_active: int, i: int, dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	"""La fonction colonne_to_dico prend un plateau, deux entiers, un dictionnaire d'options et retourne un dictionnaire.

	Si joueuse_active est impaire, on regarde la i ème ligne de plateau (sinon la i ème colonne). On regarde les cartes présente dans cette ligne (ou colonne) et on note le nombre d'occurrences de chaque cartes dans le dictionnaire retourné. Attention le plateau peut contenir des 'None'.
	"""
	dico = init_dico_cartes(dico_options)
	n = plateau.__len__()
	for j in range(n):
		# odd -> row i, even -> column i
		cell = plateau[i][j] if joueuse_active % 2 == 1 else plateau[j][i]
		if cell is not None:
			dico[cell] += 1
	return dico


@typechecked
def score_ligne(dico_ligne: dict[int, int]) -> int:
	"""Étant donné une ligne (ou une colonne) sous la forme d'un dictionnaire qui recense le nombre d'occurrences de chaque carte.

	Compter le score de la ligne en accord avec les règles du jeux Robot Master.
	"""
	# 1 copy = face value, 2 copies = 10 * face value, 3+ copies = 100 flat
	score = 0
	for card, count in dico_ligne.items():
		match count:
			case 0:
				continue
			case 1:
				score += card
			case 2:
				score += 10 * card
			case _:
				score += 100
	return score


@typechecked
def score_joueuse(plateau: Grid, joueuse_active: int, dico_options: dict[str, int] = {"maxC": 5}) -> tuple[int, int]:
	"""La fonction score_joueuse retourne un tuple d'entiers.

	Si joueuse_active est paire, on regarde le score des colone, sinon des ligne. On retourne le score ainsi que l'indice de la colonne (ou la ligne) qui réalise ce score.
	"""
	n = plateau.__len__()
	assert n > 0, "empty board has no score"
	# the score that "wins" for the player is the minimum across their lines/columns
	best_score = score_ligne(colonne_to_dico(plateau, joueuse_active, 0, dico_options))
	best_idx = 0
	for i in range(1, n):
		s = score_ligne(colonne_to_dico(plateau, joueuse_active, i, dico_options))
		if s < best_score:
			best_score = s
			best_idx = i
	return (best_score, best_idx)


@typechecked
def victoire(plateau: Grid, dico_options: dict[str, int] = {"maxC": 5}) -> tuple[int, int, int, int]:
	"""La fonction victoire retourne un tuple d'entiers.

	Contenant : le score de la joueuse, l'indice de la colonne correspondante, le score du joueur, l'indice de la ligne correspondante.
	"""
	score_j, idx_j = score_joueuse(plateau, 0, dico_options)
	score_a, idx_a = score_joueuse(plateau, 1, dico_options)
	return (score_j, idx_j, score_a, idx_a)
