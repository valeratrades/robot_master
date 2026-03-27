from __future__ import annotations

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.e_jeu import jeux
from tui_core import tui


def main() -> None:
	match ("-m" in sys.argv, "-r" in sys.argv):
		case (True, False):
			mode = "m"
		case (False, True):
			mode = "r"
		case _:
			print("usage: python -m partie_guidee (-m | -r)", file=sys.stderr)
			sys.exit(1)
	tui(jeux, mode)


main()
