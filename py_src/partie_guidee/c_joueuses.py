# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))

import random

from partie_guidee.a_plateau import Grid, plateau_to_string
from partie_guidee.b_gestionCartes import emplacement_jouable, place_carte
from typeguard import typechecked

DicoJoueuse = dict[int, tuple[str, str, dict[int, int]]]


#Donné aux étudiants
@typechecked
def init_tuple_joueuses(dico_options: dict[str, bool] = {"v": True}) -> tuple[str, str]:
	"""La fonction init_tuple_joueuses prend un argument optionnel dico_options (par défaut la clé 'v' est Vrai).

	Si v est vrai la fonction demande à l'utilisateur de saisir le nom de la joueuse et du joueur et les renvoie sous forme de tuples. Sinon, elle renvoie (Alice,Bob).
	"""
	if not dico_options.get("v", True):
		return ("Alice", "Bob")
	# > user input in a unit-tested function
	name_a = input("Nom de la joueuse : ")
	name_b = input("Nom du joueur : ")
	return (name_a, name_b)


@typechecked
def configuration_textuel(tuple_joueuses: tuple[str, str]) -> DicoJoueuse:
	"""La fonction configuration_textuel prend en argument deux noms (sous forme de tuple).

	Elle demande le mode de jeu choisi pour la joueuse / le joueur, à savoir manuel ou random.
	Elle renvoie le dictionnaire des joueuses, ou chaque personne correspond à une clé 0 ou 1, et est associé à un tuple [nom,mode de jeu,dico_main], où dico_main est initialisé au dictionnaire vide.
	"""
	# > tuple
	dico: DicoJoueuse = dict()
	for i in range(2):
		mode = input(f"Mode de jeu pour {tuple_joueuses[i]} (m/r) : ")
		dico[i] = (tuple_joueuses[i], mode, {})
	return dico


@typechecked
def choix_carte_manuel(plateau: Grid, dico_main: dict[int, int], nom_joueuse: str, dico_options: dict[str, int | bool]) -> tuple[int, int, int]:
	"""La fonction choix_carte_manuel retourne un tuple (carte,posL,posC).

	Elle demande une carte à la joueuse (en lui affichant sa main), puis lui demande un emplacement (ligne colonne) où placer sa carte.
	Si l'utilisateur rentre une information incorrect (une lettre et non un int), ou une carte non existante, ou un emplacement non jouable, la fonction ne doit pas crasher mais redemander à l'utilisateur. Voir l'utilisation de try et except.
	On fera attention a afficher le plateau et la main de la joueuse pour qu'elle puisse prendre une décision éclairée.
	"""
	# loop until we get valid input. Each iteration redraws the board,
	# erases the previous attempt, and shows warning from last error if any
	lines_to_erase = 0
	warning: str | None = None
	while True:
		if lines_to_erase > 0:
			sys.stdout.write(f"\x1B[{lines_to_erase}A\x1B[J")
		print(plateau_to_string(plateau))
		print(f"{nom_joueuse}, votre main : {dico_main}")
		if warning is not None:
			print(f"\x1B[33mWARNING: {warning}\x1B[0m")
		# board + hand + optional warning
		board_lines = plateau_to_string(plateau).count("\n") + 2 + (1 if warning is not None else 0)
		prompt_lines = 0
		warning = None
		try:
			carte = int(input("Choisissez une carte : "))
			prompt_lines += 1
		except ValueError:
			warning = "expected a number"
			lines_to_erase = board_lines + 1
			continue
		if carte not in dico_main or dico_main[carte] <= 0:
			warning = f"no card {carte} in hand"
			lines_to_erase = board_lines + 1
			continue
		try:
			posL = int(input("Ligne : "))
			prompt_lines += 1
			posC = int(input("Colonne : "))
			prompt_lines += 1
		except ValueError:
			warning = "expected a number"
			lines_to_erase = board_lines + prompt_lines + 1
			continue
		if not emplacement_jouable(plateau, posL, posC):
			n = plateau.__len__()
			if posL < 0 or posL >= n or posC < 0 or posC >= n:
				warning = f"({posL},{posC}) is out of bounds"
			elif plateau[posL][posC] is not None:
				warning = f"({posL},{posC}) is already occupied"
			else:
				warning = f"({posL},{posC}) has no adjacent card"
			lines_to_erase = board_lines + prompt_lines
			continue
		return (carte, posL, posC)


@typechecked
def choix_carte_random(plateau: Grid, dico_main: dict[int, int], nom_joueuse: str, dico_options: dict[str, int | bool]) -> tuple[int, int, int]:
	"""La fonction choix_carte_random retourne un tuple (carte,posL,posC) choisit aléatoirement parmi les cartes de la main et les positions jouables."""
	available_cards = [c for c, count in dico_main.items() if count > 0]
	n = len(plateau)
	playable_positions = [(i, j) for i in range(n) for j in range(n) if emplacement_jouable(plateau, i, j)]
	# pick randomly from the cartesian product.
	carte = random.choice(available_cards)
	posL, posC = random.choice(playable_positions)
	return (carte, posL, posC)


@typechecked
def choix_et_pose_carte(plateau: Grid, dico_joueuses: DicoJoueuse, dico_options: dict[str, int | bool], joueuse_active: int) -> None:
	"""La fonction choix_et_pose_carte effectue le tour de la joueuse_active (un int égal à 0 ou 1).

	Elle appel la fonction choix_carte_manuel ou choix_carte_random en fonction des information dans dico_joueuse, la fonction place_carte du fichier b et retire la carte du la main de la joueuse. Enfin, si la valeur de 'v' est vrai dans dico_options, on affiche un message comme 'A pose la carte x sur la case i,j' en remplaçant Axij bien évidement.
	"""
	nom, mode, dico_main = dico_joueuses[joueuse_active]

	# no enum, just a bare string. Wonderful
	if mode == "m":
		carte, posL, posC = choix_carte_manuel(plateau, dico_main, nom, dico_options)
	else:
		carte, posL, posC = choix_carte_random(plateau, dico_main, nom, dico_options)

	place_carte(plateau, posL, posC, carte)
	dico_main[carte] -= 1

	if dico_options.get("v"):
		print(f"{nom} pose la carte {carte} sur la case {posL},{posC}")
