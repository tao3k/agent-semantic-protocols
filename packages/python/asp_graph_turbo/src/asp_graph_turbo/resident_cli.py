"""Standalone Graph Turbo resident IPC process entrypoint."""

from __future__ import annotations

from .resident_server import main as serve_resident


def main() -> int:
    """Run the v1 JSON-lines IPC resident until stdin closes."""

    return serve_resident()
