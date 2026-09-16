# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Classify direct and transitive Rust consumption of registered schemas."""

from __future__ import annotations

from collections import defaultdict
from pathlib import Path
import subprocess
from typing import Any

from .catalog import SchemaDocument


EXCLUDED_RUST_SOURCE_PARTS = {
    ".cache",
    ".devenv",
    ".direnv",
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".venv",
    "build",
    "node_modules",
    "target",
}


def _git_rust_paths(workspace_root: Path) -> list[Path] | None:
    try:
        result = subprocess.run(
            [
                "git",
                "-C",
                str(workspace_root),
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "--",
                "*.rs",
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if result.returncode != 0:
        return None
    return [workspace_root / line for line in result.stdout.splitlines() if line]


def _rust_paths(workspace_root: Path) -> list[Path]:
    git_paths = _git_rust_paths(workspace_root)
    if git_paths is not None:
        return git_paths
    return [
        path
        for path in workspace_root.rglob("*.rs")
        if not EXCLUDED_RUST_SOURCE_PARTS.intersection(
            path.relative_to(workspace_root).parts
        )
    ]


def _workspace_rust_sources(workspace_root: Path) -> list[tuple[str, str]]:
    sources: list[tuple[str, str]] = []
    for path in sorted(_rust_paths(workspace_root)):
        relative = path.relative_to(workspace_root)
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        sources.append((relative.as_posix(), text))
    return sources


def _schema_tokens(
    documents: list[SchemaDocument],
) -> dict[str, list[tuple[str, str]]]:
    tokens: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for document in documents:
        candidates = (
            ("schema-file", document.path.name),
            ("schema-id", document.schema_identifier),
            ("protocol-schema-id", document.protocol_schema_id),
        )
        for kind, token in candidates:
            if token:
                tokens[token].append((document.relative_path, kind))
    return tokens


def rust_usage(
    workspace_root: Path,
    documents: list[SchemaDocument],
    edges: dict[str, set[str]],
) -> tuple[dict[str, str], dict[str, list[dict[str, Any]]]]:
    evidence: dict[str, list[dict[str, Any]]] = {
        document.relative_path: [] for document in documents
    }
    rust_sources = _workspace_rust_sources(workspace_root)

    direct: set[str] = set()
    tokens = _schema_tokens(documents)
    for source_path, text in rust_sources:
        for token, owners in tokens.items():
            if token not in text:
                continue
            for schema_path, kind in owners:
                direct.add(schema_path)
                item = {"kind": kind, "path": source_path}
                if item not in evidence[schema_path]:
                    evidence[schema_path].append(item)

    transitive: set[str] = set()
    frontier = list(sorted(direct))
    visited = set(direct)
    while frontier:
        source = frontier.pop()
        for target in sorted(edges.get(source, ())):
            if target not in direct:
                transitive.add(target)
                item = {"kind": "transitive-ref", "path": source}
                if item not in evidence[target]:
                    evidence[target].append(item)
            if target not in visited:
                visited.add(target)
                frontier.append(target)

    states = {
        document.relative_path: (
            "direct"
            if document.relative_path in direct
            else "transitive"
            if document.relative_path in transitive
            else "unobserved"
        )
        for document in documents
    }
    for items in evidence.values():
        items.sort(key=lambda item: (item["path"], item["kind"]))
    return states, evidence
