# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Contractual filename namespace family tests."""

import json
from pathlib import Path

from asp_schema_manager.catalog import load_catalog
from asp_schema_manager.families import (
    classify_schema_families,
    definition_visibility_diagnostics,
)
from asp_schema_manager.references import discover_reference_opportunities


SHAPE = {
    "type": "object",
    "additionalProperties": False,
    "required": ["value", "owner", "revision"],
    "properties": {
        "value": {"type": "string", "minLength": 1, "maxLength": 256},
        "owner": {"type": "string", "minLength": 1, "maxLength": 128},
        "revision": {"type": "integer", "minimum": 0},
    },
}


def _write_schema(path: Path, identifier: str, value: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"$id": identifier, **value}), encoding="utf-8")


def test_more_specific_filename_namespace_wins_by_contract_priority(
    tmp_path: Path,
) -> None:
    _write_schema(
        tmp_path / "schemas/semantic-query-packet.v1.schema.json",
        "https://example/semantic-query-packet",
        {"type": "object"},
    )
    documents, _diagnostics = load_catalog(tmp_path)
    families = [
        {
            "familyId": "asp.schema-family.semantic",
            "priority": 100,
            "namespace": {"filenamePrefixes": ["semantic-"]},
        },
        {
            "familyId": "asp.schema-family.semantic-search",
            "priority": 200,
            "namespace": {"filenamePrefixes": ["semantic-search-"]},
        },
    ]

    assignments, diagnostics = classify_schema_families(documents, families)

    assert diagnostics == []
    assert assignments[documents[0].relative_path] == {
        "familyId": "asp.schema-family.semantic-search",
        "familySource": "namespace-selector",
    }


def test_agent_semantic_client_namespace_is_distinct_from_semantic_agent(
    tmp_path: Path,
) -> None:
    _write_schema(
        tmp_path
        / "schemas/agent-semantic-client-cache-manifest.v1.schema.json",
        "https://example/client-cache",
        {"type": "object"},
    )
    _write_schema(
        tmp_path / "schemas/agent-semantic-client-receipt.v1.schema.json",
        "https://example/client-receipt",
        {"type": "object"},
    )
    documents, _diagnostics = load_catalog(tmp_path)
    families = [
        {
            "familyId": "asp.schema-family.semantic",
            "priority": 100,
            "namespace": {"filenamePrefixes": ["semantic-"]},
        },
        {
            "familyId": "asp.schema-family.semantic-agent",
            "priority": 200,
            "namespace": {"filenamePrefixes": ["semantic-agent-"]},
        },
        {
            "familyId": "asp.schema-family.agent-semantic-client",
            "priority": 200,
            "parentFamilyId": "asp.schema-family.semantic",
            "namespace": {"filenamePrefixes": ["agent-semantic-client-"]},
        },
    ]
    assignments, diagnostics = classify_schema_families(documents, families)
    assert diagnostics == []
    assert {item["familyId"] for item in assignments.values()} == {
        "asp.schema-family.agent-semantic-client"
    }


def test_same_priority_namespace_overlap_is_an_error(tmp_path: Path) -> None:
    _write_schema(
        tmp_path / "schemas/provider-native-request.v1.schema.json",
        "https://example/provider-native",
        {"type": "object"},
    )
    documents, _diagnostics = load_catalog(tmp_path)
    families = [
        {
            "familyId": "asp.schema-family.provider",
            "priority": 100,
            "namespace": {"filenamePrefixes": ["provider-"]},
        },
        {
            "familyId": "asp.schema-family.native",
            "priority": 100,
            "namespace": {"filenamePrefixes": ["provider-native-"]},
        },
    ]

    assignments, diagnostics = classify_schema_families(documents, families)

    assert assignments[documents[0].relative_path]["familySource"] == "ambiguous"
    assert any(item["code"] == "ambiguous-schema-family" for item in diagnostics)


def test_family_local_duplicate_targets_family_definition_contract(
    tmp_path: Path,
) -> None:
    _write_schema(
        tmp_path / "schemas/provider-a.v1.schema.json",
        "https://example/provider-a",
        {
            "$defs": {"shared": SHAPE},
            "properties": {"payload": {"$ref": "#/$defs/shared"}},
        },
    )
    _write_schema(
        tmp_path / "schemas/provider-b.v1.schema.json",
        "https://example/provider-b",
        {"properties": {"payload": SHAPE}},
    )
    documents, _diagnostics = load_catalog(tmp_path)
    families = [
        {
            "familyId": "asp.schema-family.provider",
            "priority": 100,
            "namespace": {"filenamePrefixes": ["provider-"]},
        }
    ]
    assignments, diagnostics = classify_schema_families(documents, families)

    opportunities = discover_reference_opportunities(
        documents,
        family_assignments=assignments,
        minimum_bytes=40,
    )

    assert diagnostics == []
    assert opportunities[0]["familyScope"] == "family-local"
    assert opportunities[0]["recommendation"] == (
        "extract-family-definition:asp.schema-family.provider"
    )


def test_family_definition_visibility_blocks_undeclared_cross_family_use() -> None:
    assignments = {
        "schemas/provider-request.v1.schema.json": {
            "familyId": "asp.schema-family.provider",
            "familySource": "namespace-selector",
        },
        "schemas/exact-definitions.v1.schema.json": {
            "familyId": "asp.schema-family.exact",
            "familySource": "namespace-selector",
        },
    }
    families = [
        {
            "familyId": "asp.schema-family.exact",
            "definitionSchemaPath": "schemas/exact-definitions.v1.schema.json",
            "definitionVisibility": "family",
        }
    ]

    diagnostics = definition_visibility_diagnostics(
        {
            "schemas/provider-request.v1.schema.json": {
                "schemas/exact-definitions.v1.schema.json"
            },
            "schemas/exact-definitions.v1.schema.json": set(),
        },
        assignments,
        families,
    )

    assert [item["code"] for item in diagnostics] == [
        "schema-family-definition-visibility-violation"
    ]


def test_public_family_definition_allows_cross_family_use() -> None:
    assignments = {
        "schemas/provider-request.v1.schema.json": {
            "familyId": "asp.schema-family.provider",
            "familySource": "namespace-selector",
        },
        "schemas/core-definitions.v1.schema.json": {
            "familyId": "asp.schema-family.core",
            "familySource": "namespace-selector",
        },
    }
    families = [
        {
            "familyId": "asp.schema-family.core",
            "definitionSchemaPath": "schemas/core-definitions.v1.schema.json",
            "definitionVisibility": "public",
        }
    ]

    diagnostics = definition_visibility_diagnostics(
        {
            "schemas/provider-request.v1.schema.json": {
                "schemas/core-definitions.v1.schema.json"
            },
            "schemas/core-definitions.v1.schema.json": set(),
        },
        assignments,
        families,
    )

    assert diagnostics == []
