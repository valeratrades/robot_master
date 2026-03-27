from __future__ import annotations

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).parent.parent))

from IA.e_jeu_IA import jeux
from tui_core import tui


def main() -> None:
	match ("-m" in sys.argv, "-r" in sys.argv, "-g" in sys.argv, "-a" in sys.argv, "-i" in sys.argv, "-p" in sys.argv):
		case (True, False, False, False, False, False):
			mode = "m"
		case (False, True, False, False, False, False):
			mode = "r"
		case (False, False, True, False, False, False):
			mode = "g"
		case (False, False, False, True, False, False):
			mode = "a"
		case (False, False, False, False, True, False):
			mode = "i"
		case (False, False, False, False, False, True):
			mode = "p"
		case _:
			print("usage: python -m IA (-m | -r | -g | -a | -i | -p)", file=sys.stderr)
			sys.exit(1)
	tui(jeux, mode)


main()
