# pour chercher les fonctions de partie_guidee / IA
import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.d_score import *
from IA.c_joueuses_IA import *



def tour(plateau,dico_joueuses,dico_options,joueuse_active):
	"""La fonction tour effectue un tour de jeu de la joueuse active. si l'option 'v':True est présente dans dico_options, on affichera un message comme "au tour de ..." en debut et "fin du tour" en fin de tour"""
	pass



def jeux(taille=5,maxC=5,nbC=6,v=True,cartes_distrib=12,nbJ=2):
	"""La fonction jeux() execute le jeu. Génère le plateau, les dictionnaires de joueuse et d'options, distribue les cartes et fait jouer les joueuses en alternance jusqu'à ce que le plateau soit plein. Enfin on affiche le résultats (nom de la gagnante, et les scores)"""
	pass



# Pas d'exécution hors de ce main, pour ne pas faire crasher les tests
if __name__ == "__main__": 
	jeux()