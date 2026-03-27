from __future__ import annotations

from a_plateau import cases_libres, creer_plateau, plateau_to_string
from z_variablesDeTest import Counter, new_plateau_test

plateau_test = new_plateau_test()


def test_creer_plateau_1():
	assert creer_plateau() == [[None, None, None, None, None], [None, None, None, None, None], [None, None, None, None, None], [None, None, None, None, None], [None, None, None, None, None]]


def test_creer_plateau_2():
	assert creer_plateau(9) == [[None] * 9] * 9


def test_creer_plateau_3():
	assert creer_plateau(2) == None
	assert creer_plateau(4) == None
	assert creer_plateau(6) == None


def test_creer_plateau_4():
	assert creer_plateau(-1) == None


def test_cases_libres_1():
	assert Counter(cases_libres([[None] * 3] * 3)) == Counter([(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2), (2, 0), (2, 1), (2, 2)])


def test_cases_libres_2():
	assert Counter(cases_libres(plateau_test)) == Counter([(0, 0), (0, 1), (1, 0), (1, 2), (1, 4), (2, 1), (2, 2), (2, 3), (2, 4), (3, 0), (3, 2), (3, 3)])


def test_plateau_to_string_1():
	assert plateau_to_string(creer_plateau(5)) == """-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |   |   |   |   |
(1,_)   |   |   |   |   |   |
(2,_)   |   |   |   |   |   |
(3,_)   |   |   |   |   |   |
(4,_)   |   |   |   |   |   |
-----------------------------"""


def test_plateau_to_string_2():
	assert plateau_to_string(new_plateau_test()) == """-----------------------------
          0   1   2   3   4
-----------------------------
(0,_)   |   |   | 1 | 1 | 0 |
(1,_)   |   | 2 |   | 3 |   |
(2,_)   | 4 |   |   |   |   |
(3,_)   |   | 2 |   |   | 0 |
(4,_)   | 4 | 4 | 4 | 0 | 0 |
-----------------------------"""


def test_plateau_to_string_3():
	assert plateau_to_string(creer_plateau(7)) == """-------------------------------------
          0   1   2   3   4   5   6
-------------------------------------
(0,_)   |   |   |   |   |   |   |   |
(1,_)   |   |   |   |   |   |   |   |
(2,_)   |   |   |   |   |   |   |   |
(3,_)   |   |   |   |   |   |   |   |
(4,_)   |   |   |   |   |   |   |   |
(5,_)   |   |   |   |   |   |   |   |
(6,_)   |   |   |   |   |   |   |   |
-------------------------------------"""
