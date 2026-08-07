"""Project-owned stdout emission for proof CLI entrypoints."""
from __future__ import annotations
import sys

def write_stdout(value: str, *, end: str = "\n") -> None:
    sys.stdout.write(value + end)
