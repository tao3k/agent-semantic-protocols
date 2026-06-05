"""Retired root parser compact snapshot command."""

from __future__ import annotations

import sys
from typing import Sequence

__all__ = ["main"]


def main(argv: Sequence[str] | None = None) -> int:
    del argv
    sys.stderr.write(
        "root parser compact snapshots are retired; run compact checks in the "
        "language harness that owns the provider output\n"
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
