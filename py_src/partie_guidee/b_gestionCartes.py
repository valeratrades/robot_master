# pour chercher les fonctions de partie_guidee / IA
import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import *
import random # pour la method random.shuffle(liste)
from typeguard import typechecked


# part 1 : Création et manipulation des pile / dictionnaires de cartes


@typechecked
def new_pile_cartes(dico_options: dict[str, int] = {'maxC':5,'nbC':6}) -> list[int]:
	"""La fonction new_pile_cartes crée une liste mélangée de cartes.
	La valeur la plus haute et le nombre de cartes par valeur est renseignée dans dico_options, les cartes vont de 0 à maxC inclue (5 par défaut). Il y a nbC (par défaut 6) cartes de chaque valeurs"""
	pass


@typechecked
def distribution_cartes(pile_cartes: list[int], dico_options: dict[str, int] = {'cartes_distrib':12,'nbJ':2}) -> list[int | list[int]]:
	"""La fonction distribution_cartes crée une list de nbJ+1 éléments. Le premier élément est une carte (qui sera joué au milieu), suivit nbJ listes, chacune de cartes_distrib cartes, représentant la main de chaque joueuse"""
	pass

@typechecked
def liste_to_dico(list: list[int], dico_options: dict[str, int] = {'maxC':5}) -> dict[int, int]:
	"""La fonction liste_to_dico transforme une liste de carte en un dictionnaire. Les clé du dictionnaire sont les valeurs de cartes possibles (de 0 à maxC). Et les valeurs sont le nombre de cartes correspondantes."""
	pass



# part 2 : Placement des cartes sur le plateau



@typechecked
def cases_voisines(plateau: Plateau, posL: int, posC: int) -> list[tuple[int, int]]:
	"""La fonction cases_voisines prend en argument un plateau et les coordonnées d'une case.
	Elle renvoie la liste des cases qui sont voisines de la case donnée en entrée
	La liste renvoyée est donc de taille au plus 4."""
	pass



@typechecked
def emplacement_jouable(plateau: Plateau, posL: int, posC: int) -> bool:
	"""La fonction emplacement_jouable vérifie qu'une carte peut être placée dans la position donnée. C'est à dire retourne True si la place est libre, et que au moins une case voisine est occupée."""
	pass



@typechecked
def place_carte(plateau: Plateau, posL: int, posC: int, carte: int) -> None:
	"""La fonction place_carte place la carte dans la position donnée si l'emplacement est jouable. Plateau est modifié mais rien n'est retourné."""
	pass
