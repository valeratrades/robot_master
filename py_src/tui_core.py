from __future__ import annotations

import builtins
from typing import Callable


def tui(jeux_fn: Callable[[], None], mode: str) -> None:
	"""Patch builtins.input to auto-answer name/mode prompts, then run jeux_fn."""
	answers = iter(["Alice", "Bob", mode, mode])
	real_input = builtins.input
	_sentinel = object()

	def patched_input(prompt: object = "", /) -> str:
		v = next(answers, _sentinel)
		if v is _sentinel:
			return real_input(prompt)
		assert isinstance(v, str)
		return v

	builtins.input = patched_input  # type: ignore[assignment]
	jeux_fn()
