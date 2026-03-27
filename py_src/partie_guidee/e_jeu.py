# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).parent.parent))

from IA.e_jeu_IA import tour as _tour
from IA.e_jeu_IA import jeux as _jeux
from partie_guidee.a_plateau import Grid
from partie_guidee.c_joueuses import DicoJoueuse, choix_et_pose_carte, configuration_textuel, init_tuple_joueuses
from typeguard import typechecked


@typechecked
def tour(plateau: Grid, dico_joueuses: DicoJoueuse, dico_options: dict[str, int | bool], joueuse_active: int) -> None:
	"""La fonction tour effectue un tour de jeu de la joueuse active.

	Si l'option 'v':True est présente dans dico_options, on affichera un message comme "au tour de ..." en debut et "fin du tour" en fin de tour.
	"""
	_tour(plateau, dico_joueuses, dico_options, joueuse_active, choix_fn=choix_et_pose_carte)


@typechecked
def jeux(taille: int = 5, maxC: int = 5, nbC: int = 6, v: bool = True, cartes_distrib: int = 12, nbJ: int = 2) -> None:
	"""La fonction jeux() execute le jeu.

	Génère le plateau, les dictionnaires de joueuse et d'options, distribue les cartes et fait jouer les joueuses en alternance jusqu'à ce que le plateau soit plein. Enfin on affiche le résultats (nom de la gagnante, et les scores).
	"""
	_jeux(taille, maxC, nbC, v, cartes_distrib, nbJ, choix_fn=choix_et_pose_carte, config_fn=configuration_textuel, init_fn=init_tuple_joueuses)
