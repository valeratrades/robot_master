from __future__ import annotations

from a_plateau import creer_plateau
from b_gestionCartes import cases_voisines, distribution_cartes, emplacement_jouable, liste_to_dico, new_pile_cartes, place_carte
from z_variablesDeTest import Counter, new_plateau_test, new_small_plateau_test, plateau_test, small_plateau_test


# test part 1 #############

# Test val par defaut
def test_new_pile_cartes_1():
	assert Counter(new_pile_cartes()) == Counter([0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5])


# Test taille des distributions
def test_distribution_cartes_1():
	stud = distribution_cartes(new_pile_cartes())
	assert stud[0] != None and len(stud[1]) == 12 and len(stud[2]) == 12


# Test pas de duplication ni oublis de cartes
dico_option1 = {"maxC": 6, "nbC": 1, "cartes_distrib": 3, "nbJ": 2}


def test_distribution_cartes_2():
	stud = distribution_cartes(new_pile_cartes(dico_option1), dico_option1)
	res = [stud[0]] + stud[1] + stud[2]
	assert Counter(res) == Counter([0, 1, 2, 3, 4, 5, 6])


# Test plus de paquets à distribuer
dico_option2 = {"maxC": 24, "nbC": 1, "cartes_distrib": 6, "nbJ": 4}


def test_distribution_cartes_3():
	stud = distribution_cartes(new_pile_cartes(dico_option2), dico_option2)
	res = [stud[0]] + stud[1] + stud[2] + stud[3] + stud[4]
	assert Counter(res) == Counter(list(range(25)))


# Test liste_to_dico
def test_liste_to_dico_1():
	l = [0, 1, 2, 0, 4, 5, 5, 1]
	dico = {0: 2, 1: 2, 2: 1, 3: 0, 4: 1, 5: 2}
	assert liste_to_dico(l) == dico


def test_liste_to_dico_2():
	l = [0, 2, 0, 0]
	dico = {0: 3, 1: 0, 2: 1}
	assert liste_to_dico(l, {"maxC": 2}) == dico


# test part 2 #############

# Test case voisines
def test_cases_voisines_1():
	assert Counter(cases_voisines(creer_plateau(5), 0, 0)) == Counter([(0, 1), (1, 0)])


def test_cases_voisines_2():
	assert Counter(cases_voisines(plateau_test, 0, 3)) == Counter([(1, 3), (0, 2), (0, 4)])


def test_cases_voisines_3():
	assert cases_voisines(plateau_test, -1, 5) == []


def test_cases_voisines_4():
	assert Counter(cases_voisines(plateau_test, 1, 3)) == Counter([(0, 3), (2, 3), (1, 2), (1, 4)])


# Test emplacements jouables
def test_emplacement_jouable_1():
	for (a, b, c, d) in [(plateau_test, 0, 0, False),
			(plateau_test, 2, 2, False),
			(plateau_test, 3, 4, False),
			(plateau_test, 1, 5, False),
			(plateau_test, 2, 4, True),
			(plateau_test, 2, 1, True)]:
		assert emplacement_jouable(a, b, c) == d


def test_emplacement_jouable_2():
	for (b, c, d) in [(0, 0, True),
			(4, 2, False),
			(0, 2, False),
			(1, 1, True),
			(2, 2, False)]:
		assert emplacement_jouable(small_plateau_test, b, c) == d


# Test place Carte
def test_place_carte_1():
	p = new_plateau_test()
	place_carte(p, 0, 0, 9)
	assert p == new_plateau_test()


def test_place_carte_2():
	p = new_small_plateau_test()
	place_carte(p, 1, 2, 9)
	assert p == [[None, 1, 2], [3, None, 9], [4, None, None]]
