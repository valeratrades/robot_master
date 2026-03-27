# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

import random

from partie_guidee.a_plateau import Plateau
from typeguard import typechecked

# part 1 : Création et manipulation des pile / dictionnaires de cartes


@typechecked
def new_pile_cartes(dico_options: dict[str, int] = {"maxC": 5, "nbC": 6}) -> list[int]:
	"""La fonction new_pile_cartes crée une liste mélangée de cartes.

	La valeur la plus haute et le nombre de cartes par valeur est renseignée dans dico_options, les cartes vont de 0 à maxC inclue (5 par défaut). Il y a nbC (par défaut 6) cartes de chaque valeurs
	"""
	maxC = dico_options["maxC"]
	nbC = dico_options["nbC"]
	pile = [v for v in range(maxC + 1) for _ in range(nbC)]
	random.shuffle(pile)
	return pile


@typechecked
def distribution_cartes(pile_cartes: list[int], dico_options: dict[str, int] = {"cartes_distrib": 12, "nbJ": 2}) -> list[int | list[int]]:
	"""La fonction distribution_cartes crée une list de nbJ+1 éléments.

	Le premier élément est une carte (qui sera joué au milieu), suivit nbJ listes, chacune de cartes_distrib cartes, représentant la main de chaque joueuse
	"""
	nbJ = dico_options["nbJ"]
	cartes_distrib = dico_options["cartes_distrib"]
	# First element: center card (wtf lol)
	result: list[int | list[int]] = [pile_cartes[0]]
	idx = 1
	# Deal cartes_distrib cards to each player
	for _ in range(nbJ):
		result.append(pile_cartes[idx:idx + cartes_distrib])
		idx += cartes_distrib
	return result


@typechecked
def liste_to_dico(list: list[int], dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	"""La fonction liste_to_dico transforme une liste de carte en un dictionnaire.

	Les clé du dictionnaire sont les valeurs de cartes possibles (de 0 à maxC). Et les valeurs sont le nombre de cartes correspondantes.
	"""
	maxC = dico_options["maxC"]
	dico = {v: 0 for v in range(maxC + 1)}
	for carte in list:
		dico[carte] += 1
	return dico


# part 2 : Placement des cartes sur le plateau


@typechecked
def cases_voisines(plateau: Plateau, posL: int, posC: int) -> list[tuple[int, int]]:
	"""La fonction cases_voisines prend en argument un plateau et les coordonnées d'une case.

	Elle renvoie la liste des cases qui sont voisines de la case donnée en entrée
	La liste renvoyée est donc de taille au plus 4.
	"""
	n = plateau.__len__()
	# Out of bounds => no neighbors
	if posL < 0 or posL >= n or posC < 0 or posC >= n:
		return []
	voisines = []
	for dL, dC in [(-1, 0), (1, 0), (0, -1), (0, 1)]:
		nL, nC = posL + dL, posC + dC
		if 0 <= nL < n and 0 <= nC < n:
			voisines.append((nL, nC))
	return voisines


@typechecked
def emplacement_jouable(plateau: Plateau, posL: int, posC: int) -> bool:
	"""La fonction emplacement_jouable vérifie qu'une carte peut être placée dans la position donnée.

	C'est à dire retourne True si la place est libre, et que au moins une case voisine est occupée.
	"""
	n = plateau.__len__()
	# Out of bounds or cell already occupied
	if posL < 0 or posL >= n or posC < 0 or posC >= n:
		return False
	if plateau[posL][posC] is not None:
		return False

	# At least one neighbor must be occupied
	for (vL, vC) in cases_voisines(plateau, posL, posC):
		if plateau[vL][vC] is not None:
			return True
	return False


@typechecked
def place_carte(plateau: Plateau, posL: int, posC: int, carte: int) -> None:
	"""La fonction place_carte place la carte dans la position donnée si l'emplacement est jouable.

	Plateau est modifié mais rien n'est retourné.
	"""
	if emplacement_jouable(plateau, posL, posC):
		plateau[posL][posC] = carte
