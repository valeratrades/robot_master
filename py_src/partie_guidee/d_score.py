from __future__ import annotations

import robot_master as _rc

Grid = list[list[int | None]]


def init_dico_cartes(dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	return _rc.init_dico_cartes(dico_options)


def colonne_to_dico(plateau: Grid, joueuse_active: int, i: int, dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	return _rc.colonne_to_dico(plateau, joueuse_active, i, dico_options)


def score_ligne(dico_ligne: dict[int, int]) -> int:
	return _rc.score_ligne_py(dico_ligne)


def score_joueuse(plateau: Grid, joueuse_active: int, dico_options: dict[str, int] = {"maxC": 5}) -> tuple[int, int]:
	return _rc.score_joueuse(plateau, joueuse_active, dico_options)


def victoire(plateau: Grid, dico_options: dict[str, int] = {"maxC": 5}) -> tuple[int, int, int, int]:
	return _rc.victoire_py(plateau, dico_options)
