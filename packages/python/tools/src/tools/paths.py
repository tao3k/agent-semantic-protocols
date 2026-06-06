"""Repository path helpers shared by Python tooling modules."""

from __future__ import annotations

from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parents[5]
