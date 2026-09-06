# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Deterministic schema logical projection and semantic-proof composition tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from asp_schema_manager.catalog import SchemaDocument
from asp_schema_manager.logical_projection import build_proof_plan


def _document(path: str, value: dict) -> SchemaDocument:
    return SchemaDocument(
        path=Path(path),
        relative_path=path,
        value=value,
        schema_identifier=value.get("$id"),
        protocol_schema_id=None,
        references=tuple(
            item["$ref"]
            for item in value.get("allOf", [])
            if isinstance(item, dict) and isinstance(item.get("$ref"), str)
        ),
        byte_length=len(json.dumps(value).encode()),
    )


def _state(path: str, family: str) -> dict:
    return {
        "schemaPath": path,
        "familyId": family,
        "familySource": "namespace-selector",
        "lifecycleStatus": "active",
        "lifecycleSource": "inferred",
    }


def _fixture_catalog():
    root = _document(
        "schemas/example.v1.schema.json",
        {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.test/example.v1.schema.json",
            "title": "Example",
            "type": "object",
            "properties": {"name": {"type": "string", "format": "custom"}},
            "allOf": [{"$ref": "shared.v1.schema.json"}],
            "customKeyword": True,
        },
    )
    shared = _document(
        "schemas/shared.v1.schema.json",
        {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.test/shared.v1.schema.json",
            "type": "string",
        },
    )
    documents = [root, shared]
    edges = {root.relative_path: {shared.relative_path}, shared.relative_path: set()}
    states = [
        _state(root.relative_path, "asp.schema-family.example"),
        _state(shared.relative_path, "asp.schema-family.example"),
    ]
    return documents, edges, states


def test_plan_is_deterministic_and_binds_reference_closure() -> None:
    documents, edges, states = _fixture_catalog()
    first = build_proof_plan(
        documents,
        edges,
        states,
        schema_paths=["schemas/example.v1.schema.json"],
    )
    second = build_proof_plan(
        list(reversed(documents)),
        edges,
        list(reversed(states)),
        schema_paths=["schemas/example.v1.schema.json"],
    )

    assert first == second
    assert first["validity"]["state"] == "current"
    projected = first["schemas"][0]
    assert [item["schemaPath"] for item in projected["resolvedReferenceClosure"]] == [
        "schemas/shared.v1.schema.json"
    ]
    assert projected["sourceContentDigest"].startswith("sha256:")
    assert projected["resolvedReferenceClosureDigest"].startswith("sha256:")


def test_supported_and_unsupported_keywords_create_open_obligations() -> None:
    documents, edges, states = _fixture_catalog()
    plan = build_proof_plan(documents, edges, states)
    projected = next(
        item
        for item in plan["schemas"]
        if item["sourceSchema"] == "schemas/example.v1.schema.json"
    )

    assert "type" in projected["keywordObligations"]["supported"]
    assert {"customKeyword", "format", "title"}.issubset(
        projected["keywordObligations"]["unsupported"]
    )
    dispositions = {
        item["fields"]["keywordDisposition"] for item in projected["obligations"]
    }
    assert dispositions == {"supported", "unsupported"}
    assert all(item["status"] == "open" for item in projected["obligations"])


def test_projection_and_obligations_validate_against_existing_contracts() -> None:
    workspace_root = Path(__file__).resolve().parents[5]
    documents, edges, states = _fixture_catalog()
    plan = build_proof_plan(
        documents,
        edges,
        states,
        schema_paths=["schemas/example.v1.schema.json"],
    )
    projected = plan["schemas"][0]
    definitions = json.loads(
        (
            workspace_root / "schemas/semantic-proof-definitions.v1.schema.json"
        ).read_text()
    )
    logical_projection_schema = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": definitions["$defs"],
        "$ref": "#/$defs/schemaProjection",
    }
    Draft202012Validator(logical_projection_schema).validate(
        projected["logicalProjection"]
    )
    obligation_schema = json.loads(
        (
            workspace_root / "schemas/semantic-proof-obligation.v1.schema.json"
        ).read_text()
    )
    for obligation in projected["obligations"]:
        Draft202012Validator(obligation_schema).validate(obligation)


def test_expected_digest_mismatch_is_stale() -> None:
    documents, edges, states = _fixture_catalog()
    plan = build_proof_plan(
        documents,
        edges,
        states,
        schema_paths=["schemas/example.v1.schema.json"],
        expected_plan_digest="sha256:" + "0" * 64,
    )

    assert plan["validity"]["state"] == "stale"
    assert plan["validity"]["reasonKind"] == "schema-proof-plan-digest-mismatch"


def test_unknown_selected_schema_is_rejected() -> None:
    documents, edges, states = _fixture_catalog()
    try:
        build_proof_plan(
            documents,
            edges,
            states,
            schema_paths=["schemas/missing.v1.schema.json"],
        )
    except ValueError as error:
        assert str(error) == "unknown registered schema: schemas/missing.v1.schema.json"
    else:
        raise AssertionError("missing schema selection was accepted")
