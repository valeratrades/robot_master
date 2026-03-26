from z_variablesDeTest import *
from d_score import *


def test_init_dico_cartes():
	assert init_dico_cartes()=={1: 0, 2: 0, 3: 0, 4: 0, 5: 0, 0: 0}
	dico_options={'maxC':2}
	assert init_dico_cartes(dico_options)=={0: 0, 1: 0, 2: 0}

def test_redondant(): #Oups, y a un peu de duplicats
	dico_options={'maxC':4}
	assert init_dico_cartes(dico_options) == liste_to_dico([],dico_options)

def test_colonne_to_dico_1():
	p = new_plateau_test()
	assert colonne_to_dico(p,1,1) == {0: 0, 1: 0, 2: 1, 3: 1, 4: 0, 5: 0}

def test_colonne_to_dico_2():
	p = new_plateau_test()
	assert colonne_to_dico(p,0,1) == {0: 0, 1: 0, 2: 2, 3: 0, 4: 1, 5: 0}

def test_score_ligne_1():
	assert score_ligne({0: 2, 1: 1, 2: 4, 3: 0, 4: 0, 5: 2})==151

def test_score_ligne_2():
	assert score_ligne({0: 3, 1: 1, 2: 2, 3: 0, 4: 0, 5: 0})==121

def test_score_joueuse_1():
	assert score_joueuse(new_plateau_test(),0) == (4,3)

def test_score_joueuse_2():
	assert score_joueuse(new_plateau_test(),1) == (2,3)

def test_victoire():
	assert victoire(new_plateau_test())==(4,3,2,3)