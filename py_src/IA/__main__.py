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

	if mode == "i":
		from IA import i_meilleurIA
		if "--let-it-burn" in sys.argv:
			sims = int(sys.argv[sys.argv.index("--let-it-burn") + 1])
			if sims < i_meilleurIA.MCTS_DEPTH_DEFAULT:
				print(
					f"warning: --let-it-burn {sims} is below the default ({i_meilleurIA.MCTS_DEPTH_DEFAULT}); going that low is unnecessary",
					file=sys.stderr,
				)
			i_meilleurIA._mcts_sims = sims
		from IA.e_jeu_IA import jeux
		from IA.c_joueuses_IA import configuration_textuel

		def _config_meilleur(names: tuple[str, str]):  # type: ignore[return]
			from partie_guidee.c_joueuses import DicoJoueuse
			dico: DicoJoueuse = {0: (names[0], "i", {}), 1: (names[1], "i", {})}
			return dico

		jeux(config_fn=_config_meilleur)
		return

	if mode == "p":
		raise NotImplementedError("mode 'p' is not implemented")

	algo = MODE_MAP[mode]
	bin = Path(__file__).resolve().parents[2] / "target" / "debug" / "robot_master"
	subprocess.run([str(bin), "tui", "-a", algo, "-b", algo], check=True)  #noqa: S603


main()
