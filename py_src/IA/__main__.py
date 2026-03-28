from __future__ import annotations

import subprocess
import sys
from pathlib import Path

MODE_MAP = {"m": "manual", "r": "random", "g": "greedy", "a": "sadist"}

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

	if mode in ("i", "p"):
		raise NotImplementedError(f"mode {mode!r} is not implemented")

	algo = MODE_MAP[mode]
	bin = Path(__file__).resolve().parents[2] / "target" / "debug" / "robot_master"
	subprocess.run([str(bin), "tui", "-a", algo, "-b", algo], check=True)  #noqa: S603


main()
