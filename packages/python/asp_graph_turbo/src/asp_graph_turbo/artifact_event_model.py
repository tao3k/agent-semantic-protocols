"""Artifact event data model for graph turbo timeline audits."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ArtifactEvent:
    timestamp: float
    kind: str
    language: str
    method: str
    target: str
    query: str
    project_root: str
    project_root_arg: str
    path: str
    bytes: int

    @property
    def action(self) -> bool:
        return self.kind in {"command", "query", "search", "tree-sitter-query"}
