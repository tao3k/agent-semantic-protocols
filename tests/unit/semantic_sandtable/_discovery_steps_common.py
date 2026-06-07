"""Discovery, schema-loading, capture, and stdin behavior tests."""

from __future__ import annotations

from pathlib import Path


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]

__all__ = [name for name in globals() if not name.startswith("__")]
