"""Shared fixtures for semantic query packet schema tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

_REPO_ROOT = Path(__file__).resolve().parents[3]


def semantic_query_minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-query-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        "method": "query/owner-items",
        "projectRoot": "/workspace/project",
        "ownerPath": "src/lib.rs",
        "query": "load|clone_value",
        "queryTerms": ["load", "clone_value"],
        "matchMode": "exact",
        "outputMode": "code",
        "queryCoverage": [
            {
                "value": "load",
                "status": "hit",
                "match": "exact",
                "matchCount": 1,
            }
        ],
        "matches": [
            {
                "name": "load",
                "kind": "fn",
                "visibility": "public",
                "doc": False,
                "location": {"path": "src/lib.rs", "lineRange": "6:6"},
                "read": "src/lib.rs:6:6",
                "code": "pub fn load() -> Thing { domain::make_thing() }",
                "projection": {
                    "mode": "compact",
                    "syntax": "save-token-rustfmt",
                    "sourceAuthority": "native-parser",
                    "compactSafety": {
                        "literalPolicy": "summarize",
                        "whitespacePolicy": "formatter-structural",
                        "normalization": "none",
                        "alignment": "parser-roundtrip",
                        "exactReadRequired": True,
                    },
                    "sourceFingerprint": "src/lib.rs:6:6:39",
                    "losslessStructure": True,
                    "exactRead": "src/lib.rs:6:6",
                    "nodeCount": 1,
                    "nodeLimit": 24,
                    "nodesTruncated": False,
                    "nodes": [
                        {
                            "id": "load",
                            "nativeId": "rust:fn:load",
                            "kind": "fn",
                            "role": "declaration",
                            "label": "load",
                            "depth": 0,
                            "read": "src/lib.rs:6:6",
                            "structuralFingerprint": "fn:declaration:load",
                            "flags": ["call", "return"],
                        }
                    ],
                    "renderedNodeIds": ["load"],
                    "renderedRows": [
                        {
                            "nodeId": "load",
                            "rowKind": "declaration",
                            "text": "pub fn load() -> Thing { domain::make_thing() }",
                            "semanticWeight": 1,
                        }
                    ],
                    "omitted": [
                        {
                            "kind": "body-detail",
                            "reason": "single-line compact projection keeps exact source behind read locator",
                            "count": 1,
                            "read": "src/lib.rs:6:6",
                        }
                    ],
                    "expandActions": [
                        {
                            "kind": "exact-read",
                            "target": "load",
                            "read": "src/lib.rs:6:6",
                            "argv": [
                                "rs-harness",
                                "query",
                                "--from-hook",
                                "direct-source-read",
                                "--selector",
                                "src/lib.rs:6:6",
                                ".",
                            ],
                            "reason": "read exact source before editing",
                        }
                    ],
                },
                "truncated": False,
            }
        ],
        "truncated": False,
        "notes": [],
    }


def schema_validator() -> Draft202012Validator:
    from unit.schema_validation import schema_validator_for

    schema_path = _REPO_ROOT / "schemas" / "semantic-query-packet.v1.schema.json"
    return schema_validator_for(schema_path)


def validation_errors(packet: dict[str, object]) -> list[str]:
    return [error.message for error in schema_validator().iter_errors(packet)]
