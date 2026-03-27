# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

# Ajoute le chemin du dossier parent au sys.path
sys.path.append(str(Path(__file__).parent.parent))


def choix_carte_greedy(plateau, dico_main, dico_options, joueuse_active):
	"""La fonction choix_carte_greedy retourne un tuple (carte,posL,posC) maximisant le score_complet de la joueuse_active."""
