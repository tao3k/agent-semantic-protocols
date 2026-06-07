"""Validate the semantic fact ontology fixture matrix."""

from __future__ import annotations

import json
import sys
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "tests" / "unit"))

from schema_validation import schema_validator_for  # noqa: E402

_SCHEMA_PATH = _ROOT / "schemas" / "semantic-fact-ontology.v1.schema.json"
_FIXTURES_PATH = _ROOT / "schemas" / "semantic-fact-ontology.fixtures.v1.json"
_REGISTRY_PATH = _ROOT / "schemas" / "semantic-language-registry.providers.v1.json"

_SOURCE_LANGUAGES = {"rust", "typescript", "python", "julia"}
_EXPECTED_IMPLS = {
    ("rust", "sequence"): "Vec",
    ("rust", "map"): "HashMap",
    ("typescript", "sequence"): "Array",
    ("typescript", "map"): "Map",
    ("python", "sequence"): "list",
    ("python", "map"): "dict",
    ("julia", "sequence"): "Vector",
    ("julia", "map"): "Dict",
}


def _load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_semantic_fact_ontology_schema_is_valid() -> None:
    Draft202012Validator.check_schema(_load_json(_SCHEMA_PATH))


def test_semantic_fact_ontology_fixture_matrix_is_cross_language() -> None:
    catalog = _load_json(_FIXTURES_PATH)

    schema_validator_for(_SCHEMA_PATH).validate(catalog)

    fixture_impls = {
        (fixture["languageId"], fixture["collectionFamily"]): fixture["collectionImpl"]
        for fixture in catalog["fixtures"]
    }
    assert fixture_impls == _EXPECTED_IMPLS

    for fixture in catalog["fixtures"]:
        nodes = fixture["nodes"]
        node_kinds = {node["kind"] for node in nodes}
        assert {"field", "type", "collection"} <= node_kinds

        field_nodes = {node["id"] for node in nodes if node["kind"] == "field"}
        edge_pairs = {
            (edge["source"], edge["relation"])
            for edge in fixture["edges"]
            if edge["source"] in field_nodes
        }
        assert any(relation == "has_type" for _, relation in edge_pairs)
        assert any(relation == "collection_of" for _, relation in edge_pairs)


def test_source_language_registry_advertises_semantic_fact_ontology() -> None:
    registry = _load_json(_REGISTRY_PATH)
    registrations = {
        language["languageId"]: {
            (schema["schemaId"], schema["schemaVersion"], schema["path"])
            for schema in language["schemas"]
        }
        for language in registry["languages"]
        if language["languageId"] in _SOURCE_LANGUAGES
    }

    assert set(registrations) == _SOURCE_LANGUAGES
    expected_schema = (
        "agent.semantic-protocols.semantic-fact-ontology",
        "1",
        "schemas/semantic-fact-ontology.v1.schema.json",
    )
    for schemas in registrations.values():
        assert expected_schema in schemas
