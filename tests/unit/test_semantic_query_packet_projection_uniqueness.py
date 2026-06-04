from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from tools.semantic_query_projection import (
    compact_code_layout_punctuation_errors,
    projection_rendered_row_errors,
    projection_uniqueness_errors,
)


_REPO_ROOT = Path(__file__).resolve().parents[2]


def semantic_query_packet_with_projection() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-query-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "method": "query/owner-items",
        "projectRoot": "/workspace/project",
        "ownerPath": "src/chain.ts",
        "query": "build",
        "queryTerms": ["build"],
        "matchMode": "exact",
        "outputMode": "code",
        "queryCoverage": [
            {"value": "build", "status": "hit", "match": "exact", "matchCount": 1}
        ],
        "matches": [
            {
                "name": "build",
                "kind": "function",
                "location": {"path": "src/chain.ts", "lineRange": "1:8"},
                "read": "src/chain.ts:1:8",
                "code": "function build\nreturn map",
                "projection": {
                    "mode": "compact",
                    "syntax": "semantic-outline",
                    "sourceAuthority": "native-parser",
                    "sourceFingerprint": "src/chain.ts:1:8:6a87d2cb",
                    "losslessStructure": True,
                    "exactRead": "src/chain.ts:1:8",
                    "nodes": [
                        {
                            "id": "build",
                            "nativeId": "ts:fn:build",
                            "kind": "function",
                            "role": "declaration",
                            "label": "function build",
                            "depth": 0,
                            "read": "src/chain.ts:1:8",
                            "structuralFingerprint": "fn/build/2",
                            "flags": ["return", "call"],
                        },
                        {
                            "id": "build:ret",
                            "parentId": "build",
                            "nativeId": "ts:return:0",
                            "kind": "return",
                            "role": "terminal",
                            "label": "return map",
                            "depth": 1,
                            "read": "src/chain.ts:2:7",
                            "structuralFingerprint": "return/call:map",
                            "flags": ["return"],
                        },
                    ],
                    "renderedNodeIds": ["build", "build:ret"],
                    "renderedRows": [
                        {
                            "nodeId": "build",
                            "rowKind": "declaration",
                            "text": "function build",
                            "semanticWeight": 1,
                        },
                        {
                            "nodeId": "build:ret",
                            "rowKind": "terminal",
                            "text": "return map",
                            "semanticWeight": 2,
                        },
                    ],
                    "omitted": [
                        {
                            "kind": "source-formatting",
                            "reason": "layout removed",
                            "count": 4,
                            "nodeId": "build",
                        }
                    ],
                    "expandActions": [
                        {
                            "kind": "exact-read",
                            "target": "build:ret",
                            "read": "src/chain.ts:2:7",
                            "reason": "expand return node before editing",
                        }
                    ],
                },
                "truncated": False,
            }
        ],
        "truncated": False,
    }


def semantic_query_schema_validator() -> Draft202012Validator:
    schema_path = _REPO_ROOT / "schemas" / "semantic-query-packet.v1.schema.json"
    return Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8")))


def test_projection_uniqueness_contract_accepts_canonical_packet() -> None:
    packet = semantic_query_packet_with_projection()

    schema_errors = list(semantic_query_schema_validator().iter_errors(packet))

    assert schema_errors == []
    assert projection_uniqueness_errors(packet) == []
    assert compact_code_layout_punctuation_errors(packet) == []


def test_compact_code_rejects_punctuation_only_lines() -> None:
    packet = semantic_query_packet_with_projection()
    packet["matches"][0]["code"] = "function build\nreturn map\n})"

    errors = "\n".join(
        [
            *compact_code_layout_punctuation_errors(packet),
            *projection_rendered_row_errors(packet),
        ]
    )

    assert "punctuation-only compact residue" in errors
    assert "renderedRows text does not match code" in errors


def test_projection_rendered_rows_reject_layout_residue() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    packet["matches"][0]["code"] = "function build\n}"
    projection["renderedRows"] = [
        {"nodeId": "build", "rowKind": "declaration", "text": "function build"},
        {"nodeId": "build:ret", "rowKind": "terminal", "text": "}"},
    ]

    errors = "\n".join(projection_rendered_row_errors(packet))

    assert "text is punctuation-only compact residue" in errors


def test_projection_rendered_rows_required_for_compact_code() -> None:
    packet = semantic_query_packet_with_projection()
    del packet["matches"][0]["projection"]["renderedRows"]

    errors = "\n".join(projection_rendered_row_errors(packet))

    assert "compact code lacks renderedRows" in errors


def test_projection_uniqueness_rejects_compact_exact_read_drift() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["exactRead"] = "src/chain.ts:1:7"

    errors = "\n".join(projection_uniqueness_errors(packet))

    assert (
        "exactRead src/chain.ts:1:7 does not match read locator src/chain.ts:1:8"
        in errors
    )


def test_projection_uniqueness_rejects_unbound_source_fingerprint() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["sourceFingerprint"] = "sha256:abc123"

    errors = "\n".join(projection_uniqueness_errors(packet))

    assert "sourceFingerprint does not include exactRead locator" in errors


def test_projection_uniqueness_rejects_duplicate_node_ids() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["nodes"].append(
        {
            "id": "build:ret",
            "nativeId": "ts:return:duplicate",
            "kind": "return",
            "role": "terminal",
            "label": "return duplicated",
            "depth": 1,
            "read": "src/chain.ts:4:4",
            "structuralFingerprint": "return/duplicate",
        }
    )

    assert "duplicate node id build:ret" in "\n".join(
        projection_uniqueness_errors(packet)
    )


def test_projection_uniqueness_rejects_unknown_parent_and_rendered_ids() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["nodes"][1]["parentId"] = "missing-parent"
    projection["renderedNodeIds"] = ["build", "missing-rendered"]

    errors = "\n".join(projection_uniqueness_errors(packet))

    assert "missing parentId missing-parent" in errors
    assert "rendered node id missing-rendered does not exist" in errors


def test_projection_uniqueness_rejects_unanchored_omitted_facts() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["omitted"] = [
        {"kind": "body-detail", "reason": "hidden without reverse navigation"}
    ]

    assert "omitted fact lacks nodeId/read" in "\n".join(
        projection_uniqueness_errors(packet)
    )


def test_projection_uniqueness_rejects_node_query_without_node_target() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["expandActions"] = [
        {
            "kind": "node-query",
            "target": "src/chain.ts:2:7",
            "argv": ["ts-harness", "search", "owner", "src/chain.ts", "items", "."],
            "reason": "node query must target a projection node, not a read locator",
        }
    ]

    schema_errors = list(semantic_query_schema_validator().iter_errors(packet))
    errors = "\n".join(projection_uniqueness_errors(packet))

    assert schema_errors == []
    assert "node-query target src/chain.ts:2:7 does not exist" in errors


def test_projection_uniqueness_rejects_exact_read_argv_selector_drift() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["expandActions"] = [
        {
            "kind": "exact-read",
            "target": "build:ret",
            "read": "src/chain.ts:2:7",
            "argv": [
                "ts-harness",
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "src/chain.ts",
                ".",
            ],
            "reason": "exact read argv must use the same read locator",
        }
    ]

    schema_errors = list(semantic_query_schema_validator().iter_errors(packet))
    errors = "\n".join(projection_uniqueness_errors(packet))

    assert schema_errors == []
    assert (
        "argv selector src/chain.ts does not match read locator src/chain.ts:2:7"
        in errors
    )


def test_schema_rejects_duplicate_rendered_node_ids() -> None:
    packet = semantic_query_packet_with_projection()
    projection = packet["matches"][0]["projection"]
    projection["renderedNodeIds"] = ["build", "build"]

    messages = [
        error.message for error in semantic_query_schema_validator().iter_errors(packet)
    ]

    assert any("non-unique elements" in message for message in messages)


def test_projection_uniqueness_does_not_mutate_fixture() -> None:
    packet = semantic_query_packet_with_projection()
    cloned = copy.deepcopy(packet)

    assert projection_uniqueness_errors(packet) == []
    assert packet == cloned


def test_parser_compact_query_fixtures_obey_projection_contract() -> None:
    fixture_root = (
        _REPO_ROOT / "tests" / "fixtures" / "parser-compact" / "expected-output"
    )
    fixture_paths = sorted(fixture_root.glob("*/*/*/query-packet.json"))

    assert fixture_paths

    validator = semantic_query_schema_validator()
    failures: list[str] = []
    for fixture_path in fixture_paths:
        packet = json.loads(fixture_path.read_text(encoding="utf-8"))
        relative_path = fixture_path.relative_to(_REPO_ROOT)
        failures.extend(
            f"{relative_path}: schema: {error.message}"
            for error in validator.iter_errors(packet)
        )
        failures.extend(
            f"{relative_path}: projection: {error}"
            for error in projection_uniqueness_errors(packet)
        )
        failures.extend(
            f"{relative_path}: compact-code: {error}"
            for error in compact_code_layout_punctuation_errors(packet)
        )

    assert failures == []
