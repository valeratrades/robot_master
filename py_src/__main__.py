from __future__ import annotations

import argparse
import subprocess  #noqa: S404 // default structure is stupid, so difficult to escape this thing
import sys
from enum import StrEnum, auto
from pathlib import Path


class Cmd(StrEnum):
	guided = auto()
	naive = auto()


def create_parser() -> argparse.ArgumentParser:
	parser = argparse.ArgumentParser(description="Robot Master")
	subparsers = parser.add_subparsers(dest="command", required=True)

	guided = subparsers.add_parser(Cmd.guided, help="Run the partie guidée TUI")
	guided_mode = guided.add_mutually_exclusive_group(required=True)
	guided_mode.add_argument("-m", "--manual", action="store_const", const="m", dest="mode", help="Manual mode (play yourself)")
	guided_mode.add_argument("-r", "--random", action="store_const", const="r", dest="mode", help="Random mode (AI plays)")

	naive = subparsers.add_parser(Cmd.naive, help="Run the IA game (supports greedy/agressif strategies)")
	naive_mode = naive.add_mutually_exclusive_group(required=True)
	naive_mode.add_argument("-m", "--manual", action="store_const", const="m", dest="mode", help="Manual mode")
	naive_mode.add_argument("-r", "--random", action="store_const", const="r", dest="mode", help="Random mode")
	naive_mode.add_argument("-g", "--greedy", action="store_const", const="g", dest="mode", help="Greedy AI")
	naive_mode.add_argument("-a", "--agressif", action="store_const", const="a", dest="mode", help="Agressif AI")
	naive_mode.add_argument("-i", "--meilleur", action="store_const", const="i", dest="mode", help="Meilleur IA")
	naive_mode.add_argument("-p", "--prof", action="store_const", const="p", dest="mode", help="Prof AI")
	naive.add_argument("--let-it-burn", type=int, metavar="SIMS", help="MCTS simulation count for meilleur IA (default: 800)")

	return parser


def main() -> None:
	parser = create_parser()
	args = parser.parse_args()

	match args.command:
		case Cmd.guided:
			cmd = [sys.executable, "-m", "partie_guidee", f"-{args.mode}"]
			subprocess.run(cmd, cwd=Path(__file__).parent, check=True) #noqa: S603
		case Cmd.naive:
			cmd = [sys.executable, "-m", "IA", f"-{args.mode}"]
			if getattr(args, "let_it_burn", None) is not None:
				cmd += ["--let-it-burn", str(args.let_it_burn)]
			subprocess.run(cmd, cwd=Path(__file__).parent, check=True) #noqa: S603


main()
