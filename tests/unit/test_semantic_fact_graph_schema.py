from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = _ROOT / "schemas" / "semantic-fact-graph.v1.schema.json"
_FIXTURES_PATH = _ROOT / "schemas" / "semantic-fact-ontology.fixtures.v1.json"
_REGISTRY_PATH = _ROOT / "schemas" / "semantic-language-registry.providers.v1.json"


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_semantic_fact_graph_schema_validates_fixture_derived_provider_packets() -> None:
    schema = _load_json(_SCHEMA_PATH)
    fixtures = _load_json(_FIXTURES_PATH)["fixtures"]
    validator = Draft202012Validator(schema)

    for fixture in fixtures:
        packet = _runtime_packet(fixture)
        errors = sorted(validator.iter_errors(packet), key=lambda error: list(error.path))
        assert not errors, [
            f"{fixture['fixtureId']} {list(error.path)}: {error.message}" for error in errors
        ]


def test_provider_registry_advertises_fact_graph_and_ontology_schemas() -> None:
    registry = _load_json(_REGISTRY_PATH)
    expected = {
        "agent.semantic-protocols.semantic-fact-graph",
        "agent.semantic-protocols.semantic-fact-ontology",
    }
    for language in registry["languages"]:
        if language["languageId"] not in {"rust", "python", "typescript", "julia"}:
            continue
        schema_ids = {schema["schemaId"] for schema in language["schemas"]}
        assert expected <= schema_ids, language["languageId"]


def test_semantic_fact_graph_schema_accepts_build_test_package_graph() -> None:
    schema = _load_json(_SCHEMA_PATH)
    validator = Draft202012Validator(schema)
    packet = {
        "schemaId": "agent.semantic-protocols.semantic-fact-graph",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": ".",
        "query": "changed cache owner affected tests",
        "nodes": [
            {
                "id": "owner:cache",
                "kind": "owner",
                "role": "path",
                "value": "src/cache.rs",
                "action": "owner",
                "path": "src/cache.rs",
                "fields": {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "semanticFactKind": "owner",
                    "provenance": "parser",
                    "confidence": "exact",
                    "freshness": "fresh",
                },
            },
            {
                "id": "package:cache",
                "kind": "package",
                "role": "crate",
                "value": "cache-crate",
                "action": "package",
                "path": "Cargo.toml",
                "fields": {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "semanticFactKind": "package",
                    "provenance": "build",
                    "confidence": "exact",
                    "freshness": "fresh",
                },
            },
            {
                "id": "build:cache-tests",
                "kind": "build",
                "role": "target",
                "value": "cargo test -p cache-crate",
                "action": "build",
                "fields": {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "semanticFactKind": "build",
                    "provenance": "build",
                    "confidence": "exact",
                    "freshness": "fresh",
                },
            },
            {
                "id": "test:cache",
                "kind": "test",
                "role": "path",
                "value": "tests/cache.rs",
                "action": "tests",
                "path": "tests/cache.rs",
                "fields": {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "semanticFactKind": "test",
                    "provenance": "test",
                    "confidence": "exact",
                    "freshness": "fresh",
                },
            },
            {
                "id": "dependency:serde",
                "kind": "dependency",
                "role": "crate",
                "value": "serde",
                "action": "deps",
                "fields": {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "semanticFactKind": "dependency",
                    "provenance": "build",
                    "confidence": "exact",
                    "freshness": "fresh",
                },
            },
        ],
        "edges": [
            {"source": "owner:cache", "target": "package:cache", "relation": "belongs_to"},
            {"source": "package:cache", "target": "build:cache-tests", "relation": "builds"},
            {"source": "build:cache-tests", "target": "test:cache", "relation": "tests"},
            {"source": "package:cache", "target": "dependency:serde", "relation": "depends_on"},
        ],
    }

    errors = sorted(validator.iter_errors(packet), key=lambda error: list(error.path))

    assert not errors, [f"{list(error.path)}: {error.message}" for error in errors]


def _runtime_packet(fixture: dict[str, Any]) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-fact-graph",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": fixture["languageId"],
        "providerId": fixture["providerId"],
        "projectRoot": ".",
        "query": fixture["queryIntent"],
        "nodes": [_runtime_node(fixture, node) for node in fixture["nodes"]],
        "edges": [
            {
                "source": edge["source"],
                "target": edge["target"],
                "relation": edge["relation"],
            }
            for edge in fixture["edges"]
        ],
    }


def _runtime_node(fixture: dict[str, Any], node: dict[str, Any]) -> dict[str, Any]:
    runtime = {
        key: value
        for key, value in node.items()
        if key
        not in {
            "languageId",
            "contextLocator",
            "field",
            "type",
            "collection",
            "provenance",
            "confidence",
            "freshness",
        }
    }
    fields: dict[str, Any] = {
        "languageId": node["languageId"],
        "providerId": fixture["providerId"],
        "semanticFactKind": node["kind"],
        "provenance": node["provenance"],
        "confidence": node["confidence"],
        "freshness": node["freshness"],
        "collectionFamily": fixture["collectionFamily"],
        "collectionImpl": fixture["collectionImpl"],
    }
    if "contextLocator" in node:
        runtime["locator"] = node.get("locator", node["contextLocator"])
        fields["contextLocator"] = node["contextLocator"]
    if "field" in node:
        fields["field"] = node["field"]
    if "type" in node:
        fields["type"] = node["type"]
    if "collection" in node:
        fields["collection"] = node["collection"]
    runtime["fields"] = fields
    return runtime
