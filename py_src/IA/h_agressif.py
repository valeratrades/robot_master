# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))


def cases_vide_ligne(dico_ligne, taille=5):
	"""Retourne le nombre de case vide dans le ligne donnée."""


def complete_et_score(dico_ligne, dico_cartes_restantes, scores_possibles, dico_options):
	"""Retourne la liste de tous les scores possibles d'une ligne potentiellement non complète."""


def tous_les_scores_possibles(dico_ligne, dico_cartes_restantes, dico_options):
	pass


def score_max_potentiel_complet_joueuse(plateau, joueuse_active, dico_options):
	pass


def choix_carte_agressif(plateau, dico_main, dico_options, joueuse_active):
	pass
