# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Measure immutable Python Graph compilation and warm resident ranking."""

from __future__ import annotations

import json
import time
from concurrent.futures import ThreadPoolExecutor

from asp_python_graphs.algorithm import rank_graph
from asp_python_graphs.generation_graph import compile_generation_graph
from asp_python_graphs.model import TypedGraph

from service_session_support import DIGEST_A, DIGEST_B


OWNER_COUNT = 4096
COLD_SAMPLES = 8
WARM_SAMPLES = 128
CONCURRENT_QUERIES = 32
GRAPH_QUERY_DEADLINE_NANOS = 500_000_000


def _payload() -> dict[str, object]:
    owners = [f"src/owner_{index:04}.py" for index in range(OWNER_COUNT)]
    relations = [
        {
            "from": {"kind": "owner", "id": owners[index]},
            "kind": "next-in-cohort",
            "to": {"kind": "owner", "id": owners[index + 1]},
        }
        for index in range(OWNER_COUNT - 1)
    ]
    relations.sort(
        key=lambda relation: json.dumps(relation, separators=(",", ":"), sort_keys=True)
    )
    return {
        "schemaId": "agent.semantic-protocols.search-generation-graph-request",
        "schemaVersion": "1",
        "identity": {
            "projectId": "project-large-workspace",
            "workspaceId": "workspace-large-workspace",
            "sourceRootDigest": DIGEST_A,
            "providerDigest": DIGEST_B,
            "schemaDigest": DIGEST_A,
            "generationCandidateDigest": DIGEST_B,
        },
        "sourceSnapshot": {
            "schemaId": "asp.source-snapshot.v1",
            "algorithm": "blake3-merkle-v1",
            "rootDigest": DIGEST_A.removeprefix("blake3-256:"),
            "sourceKind": "filesystem",
            "leafCount": OWNER_COUNT,
            "providerDigest": DIGEST_B.removeprefix("blake3-256:"),
        },
        "workspaceGeneration": {
            "rootDigest": DIGEST_A.removeprefix("blake3-256:"),
            "rootDepth": 1,
            "leafCount": OWNER_COUNT,
            "ownerCount": OWNER_COUNT,
        },
        "ownerPaths": owners,
        "relations": relations,
    }


def _distribution(samples: list[int]) -> dict[str, int]:
    ordered = sorted(samples)

    def at(percentile: int) -> int:
        return ordered[(len(ordered) - 1) * percentile // 100]

    return {
        "samples": len(ordered),
        "p50Nanos": at(50),
        "p95Nanos": at(95),
        "p99Nanos": at(99),
        "maxNanos": ordered[-1],
    }


def _rank(graph: TypedGraph, index: int) -> int:
    started = time.perf_counter_ns()
    result = rank_graph(
        graph,
        {
            "entryNodeIds": [f"owner:src/owner_{index % OWNER_COUNT:04}.py"],
            "budget": 64,
        },
        profile="owner-query",
    )
    assert result.ranked_nodes
    return time.perf_counter_ns() - started


def test_large_generation_graph_measures_cold_warm_and_concurrent_execution() -> None:
    payload = _payload()
    cold: list[int] = []
    compiled = None
    for _ in range(COLD_SAMPLES):
        started = time.perf_counter_ns()
        compiled = compile_generation_graph(payload)
        cold.append(time.perf_counter_ns() - started)
    assert compiled is not None
    graph = TypedGraph.from_packet({"graph": compiled.graph})

    warm = [_rank(graph, index) for index in range(WARM_SAMPLES)]
    concurrent_started = time.perf_counter_ns()
    with ThreadPoolExecutor(max_workers=CONCURRENT_QUERIES) as executor:
        concurrent = list(
            executor.map(
                lambda index: _rank(graph, index),
                range(CONCURRENT_QUERIES),
            )
        )
    concurrent_wall = time.perf_counter_ns() - concurrent_started

    receipt = {
        "schemaId": "agent.semantic-protocols.python-generation-graph-performance-receipt",
        "schemaVersion": "1",
        "owners": OWNER_COUNT,
        "cold": _distribution(cold),
        "warm": _distribution(warm),
        "concurrent": {
            **_distribution(concurrent),
            "queries": CONCURRENT_QUERIES,
            "wallNanos": concurrent_wall,
        },
        "artifactDigest": compiled.receipt["artifactDigest"],
    }
    print(json.dumps(receipt, separators=(",", ":"), sort_keys=True))

    assert receipt["cold"]["p95Nanos"] < 5_000_000_000  # type: ignore[index]
    assert receipt["warm"]["p99Nanos"] < GRAPH_QUERY_DEADLINE_NANOS  # type: ignore[index]
    assert receipt["concurrent"]["p99Nanos"] < GRAPH_QUERY_DEADLINE_NANOS  # type: ignore[index]
    assert concurrent_wall < 2_000_000_000
