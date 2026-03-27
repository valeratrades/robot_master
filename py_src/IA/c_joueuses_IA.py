from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Grid
from partie_guidee.b_gestionCartes import place_carte
from partie_guidee.c_joueuses import DicoJoueuse, choix_carte_manuel, choix_carte_random
from typeguard import typechecked


@typechecked
def configuration_textuel(tuple_joueuses: tuple[str, str]) -> DicoJoueuse:
	"""La fonction configuration_textuel prend en argument deux noms (sous forme de tuple).

	Elle demande le mode de jeu choisi pour la joueuse / le joueur, à savoir manuel, random, greedy ou agressif.
	Elle renvoie le dictionnaire des joueuses, ou chaque personne correspond à une clé 0 ou 1, et est associé à un tuple [nom,mode de jeu,dico_main], où dico_main est initialisé au dictionnaire vide.
	"""
	dico: DicoJoueuse = dict()
	for i in range(2):
		mode = input(f"Mode de jeu pour {tuple_joueuses[i]} (m/r/g/a) : ")
		dico[i] = (tuple_joueuses[i], mode, {})
	return dico


@typechecked
def choix_et_pose_carte(plateau: Grid, dico_joueuses: DicoJoueuse, dico_options: dict[str, int | bool], joueuse_active: int) -> None:
	"""La fonction choix_et_pose_carte effectue le tour de la joueuse_active (un int égal à 0 ou 1).

	Dispatche vers choix_carte_manuel, choix_carte_random, choix_carte_greedy ou choix_carte_agressif selon le mode.
	"""
	nom, mode, dico_main = dico_joueuses[joueuse_active]

	match mode:
		case "m":
			carte, posL, posC = choix_carte_manuel(plateau, dico_main, nom, dico_options)
		case "r":
			carte, posL, posC = choix_carte_random(plateau, dico_main, nom, dico_options)
		case "g":
			from IA.g_greedy import choix_carte_greedy
			carte, posL, posC = choix_carte_greedy(plateau, dico_main, dico_options, joueuse_active)
		case "a":
			from IA.h_agressif import choix_carte_agressif
			carte, posL, posC = choix_carte_agressif(plateau, dico_main, dico_options, joueuse_active)
		case _:
			raise ValueError(f"mode de jeu inconnu: {mode!r}")

	place_carte(plateau, posL, posC, carte)
	dico_main[carte] -= 1

	if dico_options.get("v"):
		print(f"{nom} pose la carte {carte} sur la case {posL},{posC}")
