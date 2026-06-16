"""CLI output helpers for semantic sandtable reports."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, TextIO


def emit(line: str = "", *, file: TextIO | None = None) -> None:
    """Write one CLI output line through an explicit reporting surface."""

    output = sys.stdout if file is None else file
    output.write(f"{line}\n")


def emit_text(text: str, *, file: TextIO | None = None, flush: bool = False) -> None:
    """Write exact CLI output text through an explicit reporting surface."""

    output = sys.stdout if file is None else file
    output.write(text)
    if flush:
        output.flush()


def emit_json(payload: Any) -> None:
    """Write JSON CLI output through the shared reporting surface."""

    emit(json.dumps(payload, indent=2, sort_keys=True))


def write_json_file(path: Path, payload: Any) -> None:
    """Write a JSON artifact with stable formatting."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def emit_json_line(payload: Any, *, flush: bool = False) -> None:
    """Write compact JSON-line output through the shared reporting surface."""

    emit_text(
        f"{json.dumps(payload, ensure_ascii=False, sort_keys=True)}\n",
        flush=flush,
    )
