# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shared packet fixtures for Graph-Turbo algorithm and evidence tests."""

from __future__ import annotations

import json
import subprocess
from contextlib import contextmanager, redirect_stderr, redirect_stdout
from io import StringIO
from os import environ
from pathlib import Path
from collections.abc import Iterator

from asp_python_graphs.algorithm import RankOptions, load_packet, rank_packet
from asp_python_graphs.cache_cli import main as cache_main
from asp_python_graphs.render import render_compact
from .schema_validation import schema_validator_for


def sample_graph_turbo_request() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": "search-playbook",
        "sourceSnapshot": {
            "schemaId": "asp.source-snapshot.v1",
            "algorithm": "blake3-merkle-v1",
            "rootDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sourceKind": "derived-overlay",
            "leafCount": 4,
            "providerDigest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        },
        "workspaceGeneration": {
            "rootDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "rootDepth": 0,
            "leafCount": 4,
            "ownerCount": 1,
        },
        "queryTerms": ["cache"],
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "entryNodeIds": ["query:cache"],
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
    with _patched_environment(env):
        output = render_compact(
            rank_packet(load_packet(str(packet_path)), RankOptions())
        )
    return subprocess.CompletedProcess(
        ["asp-python-graphs", "algorithm", str(packet_path)], 0, output, ""
    )


def run_graph_turbo_cache(
    args: list[str], env: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    stdout = StringIO()
    stderr = StringIO()
    with _patched_environment(env), redirect_stdout(stdout), redirect_stderr(stderr):
        returncode = cache_main(args)
    return subprocess.CompletedProcess(
        ["asp-python-graphs", "offline-cache", *args],
        returncode,
        stdout.getvalue(),
        stderr.getvalue(),
    )


@contextmanager
def _patched_environment(values: dict[str, str]) -> Iterator[None]:
    previous = dict(environ)
    environ.clear()
    environ.update(values)
    try:
        yield
    finally:
        environ.clear()
        environ.update(previous)


def cache_key(output: str) -> str:
    cache_line = next(line for line in output.splitlines() if line.startswith("cache="))
    return cache_line.rsplit("key=", 1)[1]


def validate_shared_schema(payload: object, schema_name: str) -> None:
    schema_path = Path(__file__).resolve().parents[2] / "schemas" / schema_name
    schema_validator_for(schema_path).validate(payload)
