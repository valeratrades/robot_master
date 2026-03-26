# pour chercher les fonctions de partie_guidee / IA
import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Plateau
from partie_guidee.b_gestionCartes import place_carte, emplacement_jouable, cases_voisines
import random
from typeguard import typechecked


DicoJoueuse = dict[int, list[str | dict[int, int]]]


#Donné aux étudiants
@typechecked
def init_tuple_joueuses(dico_options: dict[str, bool] = {'v':True}) -> tuple[str, str]:
	"""La fonction init_tuple_joueuses prend un argument optionnel dico_options (par défaut la clé 'v' est Vrai). Si v est vrai la fonction demande à l'utilisateur de saisir le nom de la joueuse et du joueur et les renvoie sous forme de tuples. Sinon, elle renvoie (Alice,Bob)."""
	pass



@typechecked
def configuration_textuel(tuple_joueuses: tuple[str, str]) -> DicoJoueuse:
	"""La fonction configuration_textuel prend en argument deux noms (sous forme de tuple).
	Elle demande le mode de jeu choisi pour la joueuse / le joueur, à savoir manuel ou random.
	Elle renvoie le dictionnaire des joueuses, ou chaque personne correspond à une clé 0 ou 1, et est associé à un tuple [nom,mode de jeu,dico_main], où dico_main est initialisé au dictionnaire vide."""
	pass


@typechecked
def choix_carte_manuel(plateau: Plateau, dico_main: dict[int, int], nom_joueuse: str, dico_options: dict[str, int | bool]) -> tuple[int, int, int]:
	"""La fonction choix_carte_manuel retourne un tuple (carte,posL,posC). Elle demande une carte à la joueuse (en lui affichant sa main), puis lui demande un emplacement (ligne colonne) où placer sa carte.
	Si l'utilisateur rentre une information incorrect (une lettre et non un int), ou une carte non existante, ou un emplacement non jouable, la fonction ne doit pas crasher mais redemander à l'utilisateur. Voir l'utilisation de try et except.
	On fera attention a afficher le plateau et la main de la joueuse pour qu'elle puisse prendre une décision éclairée."""
	pass


@typechecked
def choix_carte_random(plateau: Plateau, dico_main: dict[int, int], nom_joueuse: str, dico_options: dict[str, int | bool]) -> tuple[int, int, int]:
	"""La fonction choix_carte_random retourne un tuple (carte,posL,posC) choisit aléatoirement parmi les cartes de la main et les positions jouables."""
	pass

@typechecked
def choix_et_pose_carte(plateau: Plateau, dico_joueuses: DicoJoueuse, dico_options: dict[str, int | bool], joueuse_active: int) -> None:
	"""La fonction choix_et_pose_carte effectue le tour de la joueuse_active (un int égal à 0 ou 1). Elle appel la fonction choix_carte_manuel ou choix_carte_random en fonction des information dans dico_joueuse, la fonction place_carte du fichier b et retire la carte du la main de la joueuse. Enfin, si la valeur de 'v' est vrai dans dico_options, on affiche un message comme 'A pose la carte x sur la case i,j' en remplaçant Axij bien évidement."""
	pass
