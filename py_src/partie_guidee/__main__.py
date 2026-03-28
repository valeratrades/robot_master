from __future__ import annotations

import subprocess
import sys
from pathlib import Path

MODE_MAP = {"m": "manual", "r": "random"}

def main() -> None:
	match ("-m" in sys.argv, "-r" in sys.argv):
		case (True, False):
			mode = "m"
		case (False, True):
			mode = "r"
		case _:
			print("usage: python -m partie_guidee (-m | -r)", file=sys.stderr)
			sys.exit(1)

	algo = MODE_MAP[mode]
	bin = Path(__file__).resolve().parents[2] / "target" / "debug" / "robot_master"
	subprocess.run([str(bin), "tui", "-a", algo, "-b", algo], check=True)  #noqa: S603


main()
