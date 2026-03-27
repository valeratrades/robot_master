from __future__ import annotations

import robot_master_core as _rc

Grid = list[list[int | None]]


def creer_plateau(taille: int = 5) -> Grid | None:
	return _rc.creer_plateau(taille)


def cases_libres(plateau: Grid) -> list[tuple[int, int]]:
	return _rc.cases_libres(plateau)


def plateau_to_string(plateau: Grid, vide: str = "   ") -> str:
	return _rc.plateau_to_string(plateau, vide)


def afficher_coordonnees(plateau: Grid) -> None:
	n = len(plateau)
	for i in range(n):
		for j in range(n):
			print("|" + str((i, j)), end="")
		print("|")
