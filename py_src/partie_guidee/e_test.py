from __future__ import annotations

from a_plateau import plateau_to_string
from e_jeu import jeux, tour
from z_variablesDeTest import new_plateau_test


# Test manuel input correct
def test_tour(monkeypatch):
	p = new_plateau_test()
	print(p)
	print(plateau_to_string(p))
	dico_options = {"taille": 5, "maxC": 5, "nbC": 6, "v": True, "cartes_distrib": 12, "nbJ": 2}
	dico_main = {0: 0, 1: 1, 2: 0, 3: 0, 4: 0, 5: 0}
	dico_joueuses = {0: ("Alice", "m", dico_main), 1: ("Bob", "r", dico_main)}
	responses = iter(["1", "2", "4"])
	monkeypatch.setattr("builtins.input", lambda msg: next(responses))
	tour(p, dico_joueuses, dico_options, 0)
	assert p == [[None, None, 1, 1, 0], [None, 2, None, 3, None], [4, None, None, None, 1], [None, 2, None, None, 0], [4, 4, 4, 0, 0]]


import multiprocessing


def run_test_with_timeout(func, args, timeout):
	# lance une fonction 'func' avec arguments 'args' et retourne true si l'exécution est terminée avant 'timeout'
	p = multiprocessing.Process(target=func, args=args)
	p.start()
	p.join(timeout=timeout)
	if p.is_alive():
		p.terminate()
		p.join()
		return False  # Timeout occurred
	return True  # Test completed within timeout


def test_jeu_random_termine(monkeypatch):
	# test si le jeu termine en 3 secondes si toutes les entree console sont 'r'
	responses = iter(["r", "r", "r", "r", "r", "r", "r"])
	monkeypatch.setattr("builtins.input", lambda msg: next(responses))

	# Run jeux avec timeout de 3-second. (moins de 0.01s sur ma machine)
	success = run_test_with_timeout(jeux, (), timeout=3)
	assert success, "Test timed out"
