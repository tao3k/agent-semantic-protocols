"""Shared subprocess fixtures for graph turbo CLI tests."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import jsonschema


def sample_graph_turbo_request() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": "search-pipe",
        "queryTerms": ["cache"],
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["query:cache"],
        "budget": 4,
        "kindBudgets": {"item": 2, "owner": 1, "test": 1},
        "pathBudget": 3,
        "pathMaxHops": 4,
        "windowMerge": {"enabled": True, "maxGapLines": 8},
        "cache": {"enabled": True},
        "graph": {
            "nodes": [
                {
                    "id": "query:cache",
                    "kind": "query",
                    "role": "term",
                    "value": "cache",
                    "action": "lexical",
                },
                {
                    "id": "owner:src/lib.rs",
                    "kind": "owner",
                    "role": "path",
                    "value": "src/lib.rs",
                    "action": "owner",
                },
                {
                    "id": "item:cache_root",
                    "kind": "item",
                    "role": "symbol",
                    "value": "cache_root",
                    "action": "code",
                    "fields": {"locator": "src/lib.rs:1:1"},
                },
                {
                    "id": "test:cache_root",
                    "kind": "test",
                    "role": "path",
                    "value": "tests/cache.rs",
                    "action": "tests",
                },
            ],
            "edges": [
                {
                    "source": "query:cache",
                    "target": "item:cache_root",
                    "relation": "matches",
                    "weight": 1.0,
                },
                {
                    "source": "query:cache",
                    "target": "owner:src/lib.rs",
                    "relation": "matches",
                    "weight": 1.0,
                },
                {
                    "source": "owner:src/lib.rs",
                    "target": "item:cache_root",
                    "relation": "contains",
                    "weight": 1.0,
                },
                {
                    "source": "owner:src/lib.rs",
                    "target": "test:cache_root",
                    "relation": "covers",
                    "weight": 0.7,
                },
            ],
        },
    }


def changed_sample_graph_turbo_request() -> dict[str, object]:
    packet = json.loads(json.dumps(sample_graph_turbo_request()))
    packet["graph"]["nodes"][2]["value"] = "cache_branch"
    return packet


def run_graph_turbo_rank(
    packet_path: Path, env: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "rank",
            str(packet_path),
            "--format",
            "compact",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=env,
    )


def run_graph_turbo_cache(
    args: list[str], env: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-m", "asp_graph_turbo", "cache", *args],
        check=True,
        text=True,
        capture_output=True,
        env=env,
    )


def cache_key(output: str) -> str:
    cache_line = next(line for line in output.splitlines() if line.startswith("cache="))
    return cache_line.rsplit("key=", 1)[1]


def validate_shared_schema(payload: object, schema_name: str) -> None:
    schema_path = Path(__file__).resolve().parents[2] / "schemas" / schema_name
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    jsonschema.Draft202012Validator(schema).validate(payload)
