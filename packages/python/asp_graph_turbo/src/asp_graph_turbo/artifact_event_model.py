"""Artifact event data model for graph turbo timeline audits."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field


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
    argv: tuple[str, ...] = ()
    metadata: Mapping[str, object] = field(default_factory=dict)

    @property
    def action(self) -> bool:
        return self.kind in {
            "analysis-metadata",
            "command",
            "query",
            "search",
            "tree-sitter-query",
        }
