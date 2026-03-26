# pour chercher les fonctions de partie_guidee / IA
import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))


from partie_guidee.b_gestionCartes import *

from IA.g_greedy import *
from IA.h_agressif import *

import random


##### Copié coller de partie_guidee ####
def init_tuple_joueuses(dico_options={'v':True}):
	"""La fonction init_tuple_joueuses prend un argument optionnel dico_options (par défaut la clé 'v' est Vrai). Si v est vrai la fonction demande à l'utilisateur de saisir le nom de la joueuse et du joueur et les renvoie sous forme de tuples. Sinon, elle renvoie (Alice,Bob)."""
	pass


def choix_carte_manuel(plateau,dico_main,nom_joueuse,dico_options):
	"""La fonction choix_carte_manuel retourne un tuple (carte,posL,posC). Elle demande une carte à la joueuse (en lui affichant sa main), puis lui demande un emplacement (ligne colonne) où placer sa carte.
	Si l'utilisateur rentre une information incorrect (une lettre et non un int), ou une carte non existante, ou un emplacement non jouable, la fonction ne doit pas crasher mais redemander à l'utilisateur. Voir l'utilisation de try et except.
	On fera attention a afficher le plateau et la main de la joueuse pour qu'elle puisse prendre une décision éclairée."""
	pass


def choix_carte_random (plateau,dico_main,nom_joueuse,dico_options):
	"""La fonction choix_carte_random retourne un tuple (carte,posL,posC) choisit aléatoirement parmi les cartes de la main et les positions jouables."""
	pass


############# À redéfinir pour inclure stratégies greedy (g), agressif (a) ###############

def configuration_textuel(tuple_joueuses):
	"""La fonction configuration_textuel prend en argument deux noms (sous forme de tuple).
	Elle demande le mode de jeu choisi pour la joueuse / le joueur, à savoir manuel random, greedy ou agressif.
	Elle renvoie le dictionnaire des joueuses, ou chaque personne correspond à une clé 0 ou 1, et est associé à un tuple [nom,mode de jeu,dico_main], où dico_main est initialisé au dictionnaire vide."""
	pass

def choix_et_pose_carte(plateau,dico_joueuses,dico_options,joueuse_active):
	"""La fonction choix_et_pose_carte effectue le tour de la joueuse_active (un int égal à 0 ou 1). Elle appel la fonction choix_carte_manuel ou choix_carte_random en fonction des information dans dico_joueuse, la fonction place_carte du fichier b et retire la carte du la main de la joueuse. Enfin, si la valeur de 'v' est vrai dans dico_options, on affiche un message comme 'A pose la carte X sur la case i,j' en remplaçant AXij bien évidement."""
	pass
