"""Fixtures for semantic query projection uniqueness tests."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

_REPO_ROOT = Path(__file__).resolve().parents[3]


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
                    "nodeCount": 2,
                    "nodeLimit": 24,
                    "nodesTruncated": False,
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
    from unit.schema_validation import schema_validator_for

    return schema_validator_for(schema_path)


def repo_relative_path(path: Path) -> Path:
    return path.relative_to(_REPO_ROOT)
