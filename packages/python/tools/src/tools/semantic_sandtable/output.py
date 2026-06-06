"""CLI output helpers for semantic sandtable reports."""

from __future__ import annotations

import json
import sys
from typing import Any, TextIO


def emit(line: str = "", *, file: TextIO | None = None) -> None:
    """Write one CLI output line through an explicit reporting surface."""

    output = sys.stdout if file is None else file
    output.write(f"{line}\n")


def emit_json(payload: Any) -> None:
    """Write JSON CLI output through the shared reporting surface."""

    emit(json.dumps(payload, indent=2, sort_keys=True))
