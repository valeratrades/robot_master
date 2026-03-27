# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Plateau, cases_libres, creer_plateau, plateau_to_string
from partie_guidee.b_gestionCartes import distribution_cartes, liste_to_dico, new_pile_cartes, place_carte
from partie_guidee.c_joueuses import DicoJoueuse, choix_et_pose_carte, configuration_textuel, init_tuple_joueuses
from partie_guidee.d_score import victoire
from typeguard import typechecked


@typechecked
def tour(plateau: Plateau, dico_joueuses: DicoJoueuse, dico_options: dict[str, int | bool], joueuse_active: int) -> None:
	"""La fonction tour effectue un tour de jeu de la joueuse active.

	Si l'option 'v':True est présente dans dico_options, on affichera un message comme "au tour de ..." en debut et "fin du tour" en fin de tour.
	"""
	name = dico_joueuses[joueuse_active][0]
	verbose = dico_options.get("v", False)

	if verbose:
		print(f"au tour de {name}")

	choix_et_pose_carte(plateau, dico_joueuses, dico_options, joueuse_active)

	if verbose:
		print("fin du tour")


@typechecked
def jeux(taille: int = 5, maxC: int = 5, nbC: int = 6, v: bool = True, cartes_distrib: int = 12, nbJ: int = 2) -> None:
	"""La fonction jeux() execute le jeu.

	Génère le plateau, les dictionnaires de joueuse et d'options, distribue les cartes et fait jouer les joueuses en alternance jusqu'à ce que le plateau soit plein. Enfin on affiche le résultats (nom de la gagnante, et les scores).
	"""
	# init
	dico_options: dict[str, int | bool] = {
		"taille": taille,
		"maxC": maxC,
		"nbC": nbC,
		"v": v,
		"cartes_distrib": cartes_distrib,
		"nbJ": nbJ,
	}
	plateau = creer_plateau(taille)
	assert plateau is not None, f"invalid board size: {taille}"
	player_tuples = init_tuple_joueuses({"v": v})
	player_confs = configuration_textuel(player_tuples)
	deck = new_pile_cartes({"maxC": maxC, "nbC": nbC})
	dealt = distribution_cartes(deck, {"cartes_distrib": cartes_distrib, "nbJ": nbJ})

	# place center card. `dealt[0]` is always an `int`, but distribution_cartes
	# Remember it has `int` and `list[int]` both...
	center = taille // 2
	center_card = dealt[0]
	assert isinstance(center_card, int)
	place_carte(plateau, center, center, center_card)

	# deal hands to players, converting lists to dico format
	for i in range(nbJ):
		hand = dealt[i + 1]
		assert isinstance(hand, list)
		hand_dico = liste_to_dico(hand, {"maxC": maxC})
		name, mode, _ = player_confs[i]
		player_confs[i] = (name, mode, hand_dico)

	active = 0
	#LOOP: main
	while cases_libres(plateau).__len__() > 0:
		tour(plateau, player_confs, dico_options, active)
		active = (active + 1) % nbJ

	# calc results
	score_p0, idx_p0, score_p1, idx_p1 = victoire(plateau, {"maxC": maxC})
	name_p0 = player_confs[0][0]
	name_p1 = player_confs[1][0]

	match (score_p0 > score_p1) - (score_p1 > score_p0):
		case 1:
			verdict = f"{name_p0} wins!"
		case -1:
			verdict = f"{name_p1} wins!"
		case _:
			verdict = "draw!"

	# report results // only flush once
	print(
		f"{plateau_to_string(plateau)}\n"
		f"{name_p0}: score {score_p0} (column {idx_p0})\n"
		f"{name_p1}: score {score_p1} (row {idx_p1})\n"
		f"{verdict}"
	)


# Pas d'exécution hors de ce main, pour ne pas faire crasher les tests
if __name__ == "__main__":
	jeux()
