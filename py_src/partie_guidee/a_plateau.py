from typeguard import typechecked


Plateau = list[list[int | None]]


@typechecked
def creer_plateau(taille: int = 5) -> Plateau | None:
	"""La fonction creer_plateau prend en argument un entier positif taille
	Cet argument est optionnel et prend la valeur 5 par défaut
	Si taille est pair ou négatif, on envoie un message d'erreur
	Sinon on crée une matrice de dimension taille x taille remplie de None"""
	if taille < 0 or taille % 2 == 0:
		return None
	return [[None for _ in range(taille)] for _ in range(taille)]


#Fonction donnée aux étudiants
@typechecked
def afficher_coordonnees(plateau: Plateau) -> None:
	"""La fonction afficher_coordonnees prend en argument un plateau
	et affiche les coordonnées de chaque point du plateau. Cette fonction ne sera jamais
	utilisée, elle est là pour vous aider à comprendre comment sont fixées les coordonnées"""
	n = len(plateau)
	for i in range(n):
		for j in range(n):
			print("|"+str((i,j)), end ="")
		print("|")

# Décommentez les lignes suivantes puis exécutez le fichier pour visualizer un plateau et les coordonnées
# plateau = creer_plateau(5)
# afficher_coordonnees(plateau)


@typechecked
def cases_libres(plateau: Plateau) -> list[tuple[int, int]]:
	"""La fonction cases_libres prend en argument un plateau
	Elle renvoie la liste des cases vides (contenant la valeur none)"""

	#Q: is this more performant?
	#TODO: check against version without compaction. If the same, let's use numpy. This is hot path
	return [(i, j) for i in range(len(plateau)) for j in range(len(plateau[i])) if plateau[i][j] is None]


@typechecked
def plateau_to_string(plateau: Plateau, vide: str = "   ") -> str:
	"""La fonction plateau_to_string prend en argument un plateau,
	et un argument optionnel vide qui représente les cases vides.
	La fonction retourne une chaine de charactère."""
	n = len(plateau)
	sep = "-" * (4 * n + 9)
	cols = "".join(f"{j}   " for j in range(n))
	header = " " * 10 + cols.rstrip()
	lines = [sep, header, sep]
	for i in range(n):
		row = f"({i},_)   |"
		for j in range(n):
			cell = plateau[i][j]
			if cell is None:
				row += f"{vide}|"
			else:
				row += f" {cell} |"
		lines.append(row)
	lines.append(sep)
	return "\n".join(lines)
