"""Validate RFC 012 relation, flow-lite, and CodeQL evidence schemas."""

from __future__ import annotations

import copy
from pathlib import Path
from typing import Any

from unit.schema_validation import schema_validator_for

_REPO_ROOT = Path(__file__).resolve().parents[2]


def _validation_errors(schema_name: str, packet: dict[str, Any]) -> list[str]:
    schema_path = _REPO_ROOT / "schemas" / schema_name
    validator = schema_validator_for(schema_path)
    return [error.message for error in validator.iter_errors(packet)]


def native_relation_plan() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-relation-plan",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": ".",
        "packageName": "example",
        "relationPlanId": "relation-plan:src/lib.rs:parse",
        "ownerPath": "src/lib.rs",
        "sourceAuthority": "native-parser",
        "executionBackend": "native-parser",
        "adapterMode": "native-projection",
        "inputs": [{"kind": "handle", "target": "fn:parse"}],
        "relations": [
            {
                "id": "rel.1",
                "relation": "calls",
                "sourceHandle": "fn:parse",
                "targetHandle": "fn:parse_inner",
                "ownerPath": "src/lib.rs",
                "location": {"path": "src/lib.rs", "lineRange": "10:12"},
                "evidenceRefs": ["native-fact.1"],
                "sourceAuthority": "native-parser",
                "confidence": "proved",
            }
        ],
        "evidence": [
            {
                "id": "native-fact.1",
                "kind": "native-fact",
                "authority": "native-parser",
                "target": "rust:item:src/lib.rs:10:parse",
            }
        ],
        "artifacts": [],
        "omissions": [],
        "next": [{"kind": "exact-selector", "target": "src/lib.rs:10:12"}],
    }


def unavailable_codeql_flow_lite() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-flow-lite",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": ".",
        "flowId": "flow-lite:src/lib.rs:parse",
        "flowKind": "local-source-sink",
        "scope": "function",
        "ownerPath": "src/lib.rs",
        "sourceAuthority": "codeql",
        "executionBackend": "codeql",
        "adapterMode": "codeql-query",
        "sourceHandle": "param:input",
        "sinkHandle": "call:parse_inner",
        "path": [],
        "guards": [],
        "effects": [],
        "artifacts": [],
        "confidence": "unavailable",
        "omissions": [
            {
                "kind": "backend-unavailable",
                "message": "CodeQL database is not available for this project root.",
                "target": "codeql",
            }
        ],
    }


def codeql_evidence_artifact() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-codeql-evidence",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "artifactId": "codeql-evidence/flow-lite/src-lib-parse.json",
        "databaseFingerprint": "codeql-db:rust:1111111111111111",
        "queryId": "asp.flow-lite.local-source-sink",
        "queryVersion": "2026-06-05.v1",
        "generatedAt": "2026-06-05T12:00:00Z",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": ".",
        "inputHandles": ["param:input", "call:parse_inner"],
        "rowCount": 2,
        "projectRootPolicy": "local-only",
        "sourceSnapshot": {
            "kind": "git-tree",
            "fingerprint": "git-tree:2222222222222222",
        },
        "flowId": "flow-lite:src/lib.rs:parse",
        "normalizedRows": [
            {
                "id": "row.1",
                "kind": "source",
                "sourceHandle": "param:input",
                "location": {"path": "src/lib.rs", "lineRange": "10:10"},
            },
            {
                "id": "row.2",
                "kind": "sink",
                "targetHandle": "call:parse_inner",
                "location": {"path": "src/lib.rs", "lineRange": "12:12"},
            },
        ],
        "omissions": [],
    }


def unavailable_codeql_evidence_artifact() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-codeql-evidence",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "artifactId": "codeql-evidence/unavailable/src-lib-parse.json",
        "databaseFingerprint": "codeql-db:unavailable",
        "queryId": "asp.flow-lite.local-source-sink",
        "queryVersion": "2026-06-05.v1",
        "generatedAt": "2026-06-05T12:00:00Z",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": ".",
        "inputHandles": ["param:input", "call:parse_inner"],
        "rowCount": 0,
        "projectRootPolicy": "blocked",
        "sourceSnapshot": {
            "kind": "workspace",
            "fingerprint": "workspace:unavailable",
            "fields": {"reason": "codeql database unavailable"},
        },
        "flowId": "flow-lite:src/lib.rs:parse",
        "normalizedRows": [],
        "omissions": [
            {
                "kind": "backend-unavailable",
                "message": "CodeQL database is not available for this project root.",
                "target": "codeql",
                "fields": {"executionBackend": "codeql"},
            }
        ],
    }


def test_relation_plan_accepts_native_parser_relation_rows() -> None:
    assert (
        _validation_errors(
            "semantic-relation-plan.v1.schema.json",
            native_relation_plan(),
        )
        == []
    )


def test_relation_plan_rejects_prompt_inferred_authority() -> None:
    packet = copy.deepcopy(native_relation_plan())
    packet["sourceAuthority"] = "llm"

    assert any(
        "'llm' is not one of" in error
        for error in _validation_errors(
            "semantic-relation-plan.v1.schema.json",
            packet,
        )
    )


def test_relation_plan_rejects_absolute_owner_path() -> None:
    packet = copy.deepcopy(native_relation_plan())
    packet["ownerPath"] = "/tmp/project/src/lib.rs"

    assert any(
        "does not match" in error
        for error in _validation_errors(
            "semantic-relation-plan.v1.schema.json",
            packet,
        )
    )


def test_flow_lite_accepts_codeql_backend_unavailable_receipt() -> None:
    assert (
        _validation_errors(
            "semantic-flow-lite.v1.schema.json",
            unavailable_codeql_flow_lite(),
        )
        == []
    )


def test_codeql_evidence_accepts_metadata_only_artifact() -> None:
    assert (
        _validation_errors(
            "semantic-codeql-evidence.v1.schema.json",
            codeql_evidence_artifact(),
        )
        == []
    )


def test_codeql_evidence_accepts_backend_unavailable_artifact() -> None:
    packet = unavailable_codeql_evidence_artifact()

    assert packet["rowCount"] == 0
    assert packet["normalizedRows"] == []
    assert packet["omissions"][0]["kind"] == "backend-unavailable"
    assert (
        _validation_errors(
            "semantic-codeql-evidence.v1.schema.json",
            packet,
        )
        == []
    )


def test_codeql_evidence_rejects_raw_backend_output_fields() -> None:
    packet = copy.deepcopy(codeql_evidence_artifact())
    packet["rawOutput"] = [{"source": "param:input", "sink": "call:parse_inner"}]

    assert any(
        "Additional properties are not allowed" in error
        for error in _validation_errors(
            "semantic-codeql-evidence.v1.schema.json",
            packet,
        )
    )


def test_codeql_evidence_requires_relation_or_flow_owner() -> None:
    packet = copy.deepcopy(codeql_evidence_artifact())
    packet.pop("flowId")

    assert any(
        "is not valid under any of the given schemas" in error
        for error in _validation_errors(
            "semantic-codeql-evidence.v1.schema.json",
            packet,
        )
    )
