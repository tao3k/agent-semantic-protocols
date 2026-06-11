"""Tests for the ASP graph turbo Python package."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from asp_graph_turbo import (
    TypedGraph,
    rank_frontier,
    render_compact,
    result_to_packet,
)

from .schema_validation import schema_validator_for

__all__ = [
    "TypedGraph",
    "_GRAPH_TURBO_FIXTURE",
    "_GRAPH_TURBO_FEEDBACK_SCHEMA",
    "_GRAPH_TURBO_CALIBRATION_SCHEMA",
    "_GRAPH_TURBO_REQUEST_SCHEMA",
    "_GRAPH_TURBO_SCHEMA",
    "Path",
    "json",
    "rank_frontier",
    "render_compact",
    "result_to_packet",
    "sample_failure_packet",
    "sample_packet",
    "sample_request",
    "schema_validator_for",
    "subprocess",
    "sys",
]


_REPO_ROOT = Path(__file__).resolve().parents[2]
_GRAPH_TURBO_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-result.v1.schema.json"
)
_GRAPH_TURBO_REQUEST_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-request.v1.schema.json"
)
_GRAPH_TURBO_FEEDBACK_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-feedback.v1.schema.json"
)
_GRAPH_TURBO_CALIBRATION_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-calibration.v1.schema.json"
)
_GRAPH_TURBO_FIXTURE = (
    _REPO_ROOT / "sandtables" / "fixtures" / "asp" / "graph-turbo-owner-query.json"
)


def sample_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:parser", "kind": "query", "role": "term", "value": "parser"},
            {"id": "owner:cli", "kind": "owner", "role": "path", "value": "src/cli.py"},
            {
                "id": "item:collect",
                "kind": "item",
                "role": "fn",
                "value": "collect_actions",
                "owner": "src/cli.py",
                "path": "src/cli.py",
                "ownerPath": "src/cli.py",
                "symbol": "collect_actions",
                "startLine": 10,
                "endLine": 20,
                "locator": "src/cli.py:10:20",
            },
            {
                "id": "hot:command",
                "kind": "hot",
                "role": "call",
                "value": "command_intent",
                "owner": "src/cli.py",
                "path": "src/cli.py",
                "ownerPath": "src/cli.py",
                "symbol": "command_intent",
                "startLine": 24,
                "endLine": 28,
                "locator": "src/cli.py:24:28",
            },
            {
                "id": "dep:jsonschema",
                "kind": "dependency",
                "role": "pkg",
                "value": "jsonschema",
            },
            {
                "id": "test:cli",
                "kind": "test",
                "role": "path",
                "value": "tests/test_cli.py",
            },
        ],
        "edges": [
            {"source": "q:parser", "target": "owner:cli", "relation": "matches"},
            {"source": "q:parser", "target": "item:collect", "relation": "matches"},
            {"source": "owner:cli", "target": "item:collect", "relation": "contains"},
            {"source": "item:collect", "target": "hot:command", "relation": "contains"},
            {"source": "owner:cli", "target": "dep:jsonschema", "relation": "uses"},
            {"source": "owner:cli", "target": "test:cli", "relation": "covers"},
        ],
    }


def sample_request(
    *, profile: str = "owner-query", budget: int = 8
) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": "search-pipe",
        "queryTerms": ["parser"],
        "profile": profile,
        "algorithm": "typed-ppr-diverse",
        "source": "finder",
        "candidateSources": ["finder"],
        "sourceTrace": [
            {
                "source": "finder",
                "status": "used",
                "matched": 3,
                "missing": 0,
                "normalized": 3,
            }
        ],
        "seedIds": ["q:parser", "owner:cli"],
        "budget": budget,
        "kindBudgets": {"owner": 1, "dependency": 1, "test": 1},
        "windowMerge": {"enabled": True, "maxGapLines": 8},
        "pathBudget": 4,
        "pathMaxHops": 4,
        "cache": {"enabled": True},
        "graph": sample_packet(),
    }


def sample_failure_packet() -> dict[str, object]:
    return {
        "nodes": [
            {
                "id": "failure:cache",
                "kind": "failure",
                "role": "test-failure",
                "value": "cache_cli::writeback::prompt_output_replay",
                "failureKind": "test-failure",
                "languageId": "rust",
            },
            {
                "id": "assert:replay",
                "kind": "assert",
                "role": "failure",
                "value": "expected=hit,actual=miss",
                "languageId": "rust",
            },
            {
                "id": "owner:writeback",
                "kind": "owner",
                "role": "path",
                "value": "src/cache_cli/writeback.rs",
                "path": "src/cache_cli/writeback.rs",
                "languageId": "rust",
            },
            {
                "id": "hot:write",
                "kind": "hot",
                "role": "fn",
                "value": "write_prompt_output_artifact",
                "path": "src/cache_cli/writeback.rs",
                "ownerPath": "src/cache_cli/writeback.rs",
                "symbol": "write_prompt_output_artifact",
                "startLine": 10,
                "endLine": 24,
                "locator": "src/cache_cli/writeback.rs:10:24",
                "languageId": "rust",
            },
            {
                "id": "key:fingerprint",
                "kind": "key",
                "role": "signal",
                "value": "request_fingerprint",
                "languageId": "rust",
            },
            {
                "id": "evidence:file-hash",
                "kind": "evidence",
                "role": "signal",
                "value": "file_hash(observed=failure)",
                "languageId": "rust",
            },
            {
                "id": "test:writeback",
                "kind": "test",
                "role": "path",
                "value": "tests/unit/cache_cli/writeback.rs",
                "path": "tests/unit/cache_cli/writeback.rs",
                "languageId": "rust",
            },
        ],
        "edges": [
            {
                "source": "failure:cache",
                "target": "test:writeback",
                "relation": "fails",
            },
            {
                "source": "failure:cache",
                "target": "assert:replay",
                "relation": "explains",
            },
            {
                "source": "failure:cache",
                "target": "owner:writeback",
                "relation": "selects",
            },
            {"source": "assert:replay", "target": "hot:write", "relation": "checks"},
            {
                "source": "assert:replay",
                "target": "key:fingerprint",
                "relation": "checks",
            },
            {
                "source": "assert:replay",
                "target": "evidence:file-hash",
                "relation": "gates",
            },
            {
                "source": "owner:writeback",
                "target": "hot:write",
                "relation": "contains",
            },
            {"source": "hot:write", "target": "key:fingerprint", "relation": "relates"},
            {
                "source": "hot:write",
                "target": "evidence:file-hash",
                "relation": "validates",
            },
        ],
    }
