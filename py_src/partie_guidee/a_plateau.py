from typeguard import typechecked


Plateau = list[list[int | None]]


@typechecked
def creer_plateau(taille: int = 5) -> Plateau | None:
	"""La fonction creer_plateau prend en argument un entier positif taille
	Cet argument est optionnel et prend la valeur 5 par défaut
	Si taille est pair ou négatif, on envoie un message d'erreur
	Sinon on crée une matrice de dimension taille x taille remplie de None"""
	pass


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
	pass


@typechecked
def plateau_to_string(plateau: Plateau, vide: str = "   ") -> str:
	"""La fonction plateau_to_string prend en argument un plateau,
	et un argument optionnel vide qui représente les cases vides.
	La fonction retourne une chaine de charactère."""
	pass
