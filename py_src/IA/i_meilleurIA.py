# pour chercher les fonctions de partie_guidee / IA
from __future__ import annotations

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.a_plateau import Grid
from typeguard import typechecked


@typechecked
def choix_carte_IA(plateau: Grid, dico_main: dict[int, int], dico_options: dict[str, int], joueuse_active: int) -> tuple[int, int, int]:
	"""Extension "battez les profs": retourne un tuple (carte, posL, posC).

	Les profs développeront une stratégie choix_carte_prof et nous comparerons l'efficacité des IA.
	"""
	raise NotImplementedError  #dbg


@typechecked
def choix_carte_prof(plateau: Grid, dico_main: dict[int, int], dico_options: dict[str, int], joueuse_active: int) -> tuple[int, int, int]:
	"""Stratégie des profs, pour comparer l'efficacité des IA."""
	raise NotImplementedError  #dbg
