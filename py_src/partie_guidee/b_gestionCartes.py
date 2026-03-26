# pour chercher les fonctions de partie_guidee / IA
import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import *
import random # pour la method random.shuffle(liste)


# part 1 : Création et manipulation des pile / dictionnaires de cartes 


def new_pile_cartes(dico_options ={'maxC':5,'nbC':6}):
	"""La fonction new_pile_cartes crée une liste mélangée de cartes.
	La valeur la plus haute et le nombre de cartes par valeur est renseignée dans dico_options, les cartes vont de 0 à maxC inclue (5 par défaut). Il y a nbC (par défaut 6) cartes de chaque valeurs"""
	pass


def distribution_cartes(pile_cartes,dico_options={'cartes_distrib':12,'nbJ':2}):
	"""La fonction distribution_cartes crée une list de nbJ+1 éléments. Le premier élément est une carte (qui sera joué au milieu), suivit nbJ listes, chacune de cartes_distrib cartes, représentant la main de chaque joueuse"""
	pass

def liste_to_dico(list,dico_options={'maxC':5}):
	"""La fonction liste_to_dico transforme une liste de carte en un dictionnaire. Les clé du dictionnaire sont les valeurs de cartes possibles (de 0 à maxC). Et les valeurs sont le nombre de cartes correspondantes."""
	pass



# part 2 : Placement des cartes sur le plateau 



def cases_voisines(plateau,posL,posC):
	"""La fonction cases_voisines prend en argument un plateau et les coordonnées d'une case.
	Elle renvoie la liste des cases qui sont voisines de la case donnée en entrée
	La liste renvoyée est donc de taille au plus 4."""
	pass



def emplacement_jouable(plateau,posL,posC):
	"""La fonction emplacement_jouable vérifie qu'une carte peut être placée dans la position donnée. C'est à dire retourne True si la place est libre, et que au moins une case voisine est occupée."""
	pass



def place_carte(plateau,posL,posC,carte):
	"""La fonction place_carte place la carte dans la position donnée si l'emplacement est jouable. Plateau est modifié mais rien n'est retourné."""
	pass