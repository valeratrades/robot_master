# pour chercher les fonctions de partie_guidee / IA
import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from IA.f_fonctions_additionelles import *


def choix_carte_greedy (plateau,dico_main,dico_options,joueuse_active):
	"""La fonction choix_carte_greedy retourne un tuple (carte,posL,posC) choisit maximisant le score_complet de la joueuse_active."""
	pass