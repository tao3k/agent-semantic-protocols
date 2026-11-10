# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Language packages expose syntax/parser facts, not EvidenceGraph ownership."""

from __future__ import annotations

import json
from collections.abc import Iterator
from pathlib import Path
from typing import Any

_REPO_ROOT = Path(__file__).resolve().parents[2]
_LANGUAGE_PACKAGE_ROOTS = (
    _REPO_ROOT / "languages/asp-rust",
    _REPO_ROOT / "languages/asp-python",
    _REPO_ROOT / "languages/asp-typescript",
    _REPO_ROOT / "languages/AspJulia.jl",
    _REPO_ROOT / "languages/asp-gerbil-scheme",
)
_CENTRAL_ONLY_SCHEMAS = (
    "semantic-evidence-graph.v1.schema.json",
    "semantic-assurance-case.v1.schema.json",
)


def test_language_schema_bundles_do_not_own_central_graph_artifacts() -> None:
    for package_root in _LANGUAGE_PACKAGE_ROOTS:
        schema_root = package_root / "schemas"
        profile = json.loads(
            (schema_root / "language-schema-profiles.json").read_text(encoding="utf-8")
        )
        assert all(
            schema_name not in root_set
            for root_set in profile["rootSets"].values()
            for schema_name in _CENTRAL_ONLY_SCHEMAS
        ), package_root
        for schema_name in _CENTRAL_ONLY_SCHEMAS:
            assert not (schema_root / schema_name).exists(), package_root


def test_language_provider_descriptors_do_not_advertise_legacy_commands() -> None:
    forbidden = {"check", "evidence"}
    for package_root in _LANGUAGE_PACKAGE_ROOTS:
        for path in package_root.rglob("*.json"):
            try:
                document = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, UnicodeDecodeError, json.JSONDecodeError):
                continue
            advertised = {
                value
                for key, value in _walk_scalars(document)
                if key in {"command", "method"} and isinstance(value, str)
            }
            assert advertised.isdisjoint(forbidden), (
                "language provider descriptor advertises a Runtime-owned command: "
                f"{path}: {sorted(advertised & forbidden)}"
            )


def _walk_scalars(value: Any) -> Iterator[tuple[str, Any]]:
    if isinstance(value, dict):
        for key, child in value.items():
            if isinstance(child, (dict, list)):
                yield from _walk_scalars(child)
            else:
                yield key, child
    elif isinstance(value, list):
        for child in value:
            yield from _walk_scalars(child)
