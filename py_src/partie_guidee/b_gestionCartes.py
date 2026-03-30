from __future__ import annotations

import robot_master as _rc

Grid = list[list[int | None]]


def new_pile_cartes(dico_options: dict[str, int] = {"maxC": 5, "nbC": 6}) -> list[int]:
	return _rc.new_pile_cartes(dico_options)


def distribution_cartes(pile_cartes: list[int], dico_options: dict[str, int] = {"cartes_distrib": 12, "nbJ": 2}) -> list[int | list[int]]:
	return _rc.distribution_cartes(pile_cartes, dico_options)


def liste_to_dico(list: list[int], dico_options: dict[str, int] = {"maxC": 5}) -> dict[int, int]:
	return _rc.liste_to_dico(list, dico_options)


def cases_voisines(plateau: Grid, posL: int, posC: int) -> list[tuple[int, int]]:
	return _rc.cases_voisines(plateau, posL, posC)


def emplacement_jouable(plateau: Grid, posL: int, posC: int) -> bool:
	return _rc.emplacement_jouable(plateau, posL, posC)


def place_carte(plateau: Grid, posL: int, posC: int, carte: int) -> None:
	result = _rc.place_carte(plateau, posL, posC, carte)
	for i in range(len(plateau)):
		plateau[i] = result[i]
