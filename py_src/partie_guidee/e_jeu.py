# pour chercher les fonctions de partie_guidee / IA
import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Plateau
from partie_guidee.c_joueuses import DicoJoueuse, choix_et_pose_carte
from partie_guidee.d_score import victoire
from typeguard import typechecked



@typechecked
def tour(plateau: Plateau, dico_joueuses: DicoJoueuse, dico_options: dict[str, int | bool], joueuse_active: int) -> None:
	"""La fonction tour effectue un tour de jeu de la joueuse active. si l'option 'v':True est présente dans dico_options, on affichera un message comme "au tour de ..." en debut et "fin du tour" en fin de tour"""
	pass


@typechecked
def jeux(taille: int = 5, maxC: int = 5, nbC: int = 6, v: bool = True, cartes_distrib: int = 12, nbJ: int = 2) -> None:
	"""La fonction jeux() execute le jeu. Génère le plateau, les dictionnaires de joueuse et d'options, distribue les cartes et fait jouer les joueuses en alternance jusqu'à ce que le plateau soit plein. Enfin on affiche le résultats (nom de la gagnante, et les scores)"""
	pass


# Pas d'exécution hors de ce main, pour ne pas faire crasher les tests
if __name__ == "__main__":
	jeux()
