"""Console reporting helpers for package CLI commands."""

from __future__ import annotations

import sys
from typing import TextIO


def emit(message: object = "", *, file: TextIO | None = None) -> None:
    stream = sys.stdout if file is None else file
    stream.write(f"{message}\n")
