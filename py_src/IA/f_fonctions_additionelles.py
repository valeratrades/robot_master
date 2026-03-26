# pour chercher les fonctions de partie_guidee / IA
import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Plateau
from partie_guidee.d_score import score_joueuse, colonne_to_dico



# Obligatoire :
def score_complet_joueuse(plateau,joueuse_active,dico_options):
	"""retourne la liste des scores de chaque colone de la joueuse active, triée du plus petit au plus grand score"""
	pass



# Facultatif :


def copie_plateau(plateau):
	"""crée un nouveau plateau, copie de l'argument tel que modifier la sortie n'impacte pas l'entré."""
	pass


def cases_jouables(plateau):
	"""La fonction cases_jouable prend en argument un plateau.
	Elle renvoie la liste des cases jouable (vide avec une voisine non vide)"""
	pass

def cartes_jouables(dico_main):
	"""La fonction cases_jouable prend en argument un dico_main.
	Elle renvoie la liste des cartes jouable (présentes dans la main)"""
	pass




def cartes_jouees(plateau,dico_options={'maxC':5}):
	pass


def cartes_restantes(plateau,dico_options={'maxC':5,'nbC':6}):
	"""Calcul les cartes possibles restante (non déjà joué)"""
	pass