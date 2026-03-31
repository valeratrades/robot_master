# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))


from IA.g_greedy import *
from IA.h_agressif import *
from partie_guidee.b_gestionCartes import *


# test pour tous les score possible
def test_tous_les_scores_possibles():
	dico_ligne = {0: 1, 1: 1, 2: 0, 3: 0, 4: 0, 5: 1}
	dico_crest = {0: 2, 1: 0, 2: 1, 3: 0, 4: 0, 5: 1}
	dico_options = {"maxC": 5, "nbC": 6, "taille": 5}
	assert set([106, 8, 8, 51, 51, 53, 53]) == set(tous_les_scores_possibles(dico_ligne, dico_crest, dico_options))

# print(tous_les_scores_possibles(dico_ligne,dico_crest,dico_options))
# Fin du test : [106,8,8,51,51,53,53]


def test_greedy_colone():
	(p, d_m, d_o) = config1()
	assert choix_carte_greedy(p, d_m, d_o, 0) == [5, 4, 0] or choix_carte_greedy(p, d_m, d_o, 0) == (5, 4, 0)


def test_greedy_ligne():
	(p, d_m, d_o) = config1()
	assert choix_carte_greedy(p, d_m, d_o, 1) == [1, 0, 3] or choix_carte_greedy(p, d_m, d_o, 1) == (1, 0, 3)


def test_aggro_colone():
	(p, d_m, d_o) = config1()
	assert choix_carte_agressif(p, d_m, d_o, 0) == [0, 2, 1] or choix_carte_agressif(p, d_m, d_o, 0) == (0, 2, 1)


def test_aggro_ligne():
	(p, d_m, d_o) = config1()
	assert choix_carte_agressif(p, d_m, d_o, 1) == [0, 1, 2] or choix_carte_agressif(p, d_m, d_o, 1) == (0, 1, 2)


def plateau_strat1():
	return [[None, None, None, None, 1], [None, None, None, None, 5], [None, None, 3, 2, 1], [None, None, 4, 4, None], [None, 2, 2, 2, None]]


def config1():
	p = plateau_strat1()
	d_o = {"maxC": 5, "nbC": 6, "taille": 5, "v": False}
	d_m = {0: 1, 1: 2, 2: 1, 3: 1, 4: 1, 5: 1}
	return (p, d_m, d_o)


def plateau_strat2():
	return [[None, None, None, None, 1], [None, None, None, None, 5], [None, None, 3, 2, 1], [None, None, 4, 4, None], [None, 2, 2, 2, None]]


def config2():
	p = plateau_strat2()
	d_o = {"maxC": 5, "nbC": 6, "taille": 5, "v": False}
	d_m = {0: 1, 1: 2, 2: 1, 3: 1, 4: 1, 5: 1}
	return (p, d_m, d_o)
