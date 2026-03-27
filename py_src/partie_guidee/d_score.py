# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Plateau
from typeguard import typechecked


@typechecked
def init_dico_cartes(dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	"""La fonction init_dico_cartes crée un dictionnaire dont les clés sont des int correspondants aux valeurs possibles des cartes.

	Les valeurs sont initialisées à 0. maxC est une variables optionnelle donnant la valeur maximal d'une carte.
	"""


@typechecked
def colonne_to_dico(plateau: Plateau, joueuse_active: int, i: int, dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	"""La fonction colonne_to_dico prend un plateau, deux entiers, un dictionnaire d'options et retourne un dictionnaire.

	Si joueuse_active est impaire, on regarde la i ème ligne de plateau (sinon la i ème colonne). On regarde les cartes présente dans cette ligne (ou colonne) et on note le nombre d'occurrences de chaque cartes dans le dictionnaire retourné. Attention le plateau peut contenir des 'None'.
	"""


@typechecked
def score_ligne(dico_ligne: dict[int, int]) -> int:
	"""Étant donné une ligne (ou une colonne) sous la forme d'un dictionnaire qui recense le nombre d'occurrences de chaque carte.

	Compter le score de la ligne en accord avec les règles du jeux Robot Master.
	"""


@typechecked
def score_joueuse(plateau: Plateau, joueuse_active: int, dico_options: dict[str, int] = {"maxC": 5}) -> tuple[int, int]:
	"""La fonction score_joueuse retourne un tuple d'entiers.

	Si joueuse_active est paire, on regarde le score des colone, sinon des ligne. On retourne le score ainsi que l'indice de la colonne (ou la ligne) qui réalise ce score.
	"""


@typechecked
def victoire(plateau: Plateau, dico_options: dict[str, int] = {"maxC": 5}) -> tuple[int, int, int, int]:
	"""La fonction victoire retourne un tuple d'entiers.

	Contenant : le score de la joueuse, l'indice de la colonne correspondante, le score du joueur, l'indice de la ligne correspondante.
	"""
