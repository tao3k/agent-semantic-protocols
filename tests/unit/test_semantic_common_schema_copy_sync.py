"""Validate package-local copies of shared semantic schemas."""

from __future__ import annotations

import json
from pathlib import Path


_ROOT = Path(__file__).resolve().parents[2]


def _load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def test_language_package_common_schema_copies_match_protocol_root() -> None:
    schema_names = (
        "semantic-query-packet.v1.schema.json",
        "semantic-search-packet.v1.schema.json",
        "semantic-read-packet.v1.schema.json",
        "semantic-source-location.v1.schema.json",
        "semantic-tree-sitter-query.v1.schema.json",
        "semantic-tree-sitter-grammar-profile.v1.schema.json",
        "semantic-tree-sitter-provenance.v1.schema.json",
        "semantic-native-syntax-fact-index.v1.schema.json",
    )
    package_roots = (
        "languages/rust-lang-project-harness",
        "languages/typescript-lang-project-harness",
        "languages/python-lang-project-harness",
        "languages/JuliaLangProjectHarness.jl",
    )

    for package_root in package_roots:
        for schema_name in schema_names:
            root_schema = _load_json(_ROOT / "schemas" / schema_name)
            package_schema = _load_json(
                _ROOT / package_root / "schemas" / schema_name
            )
            assert package_schema == root_schema, f"{package_root}:{schema_name}"
