import argparse
import subprocess
import sys
from pathlib import Path


def create_parser() -> argparse.ArgumentParser:
	parser = argparse.ArgumentParser(description="Robot Master")
	subparsers = parser.add_subparsers(dest="command", required=True)

	tui = subparsers.add_parser("tui", help="Run the partie guidée TUI")
	mode = tui.add_mutually_exclusive_group(required=True)
	mode.add_argument("-m", "--manual", action="store_const", const="m", dest="mode", help="Manual mode (play yourself)")
	mode.add_argument("-r", "--random", action="store_const", const="r", dest="mode", help="Random mode (AI plays)")

	return parser


def main() -> None:
	parser = create_parser()
	args = parser.parse_args()

	match args.command:
		case "tui":
			cmd = [sys.executable, "-m", "partie_guidee", f"-{args.mode}"]
			subprocess.run(cmd, cwd=Path(__file__).parent, check=True)


main()
