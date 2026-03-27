from __future__ import annotations

import builtins
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).parent.parent))

from partie_guidee.e_jeu import jeux


def main() -> None:
	match ("-m" in sys.argv, "-r" in sys.argv):
		case (True, False):
			mode = "m"
		case (False, True):
			mode = "r"
		case _:
			print("usage: python -m partie_guidee (-m | -r)", file=sys.stderr)
			sys.exit(1)
	answers = iter(["Alice", "Bob", mode, "r"])
	real_input = builtins.input
	_sentinel = object()

	def patched_input(prompt: object = "", /) -> str:
		v = next(answers, _sentinel)
		if v is _sentinel:
			return real_input(prompt)
		assert isinstance(v, str)
		return v

	builtins.input = patched_input  # type: ignore[assignment]
	jeux()


main()
