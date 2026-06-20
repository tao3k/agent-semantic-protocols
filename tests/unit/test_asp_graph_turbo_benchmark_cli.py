"""Benchmark command tests for graph turbo sandtable evidence."""

from __future__ import annotations

import json
import subprocess
import sys

from unit.asp_graph_turbo_cli_support import (
    sample_graph_turbo_request,
    validate_shared_schema,
)


def test_graph_turbo_benchmark_json_is_schema_owned(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "benchmark",
            str(packet_path),
            "--runs",
            "3",
            "--warmup-runs",
            "1",
            "--cache-mode",
            "disabled",
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(completed.stdout)
    validate_shared_schema(payload, "semantic-graph-turbo-benchmark.v1.schema.json")

    assert payload["packetKind"] == "graph-turbo-benchmark"
    assert payload["runs"] == 3
    assert payload["warmupRuns"] == 1
    assert payload["cacheMode"] == "disabled"
    assert payload["durationMs"]["median"] >= 0
    assert payload["durationMs"]["p95"] >= payload["durationMs"]["median"]
    assert payload["cacheStatusCounts"] == {"disabled": 3}
    assert payload["warmupCacheStatusCounts"] == {"disabled": 1}
    assert payload["lastAlgorithmMetrics"]["cacheStatus"] == "disabled"
    assert payload["lastAlgorithmMetrics"]["depthCacheStatus"] == "disabled"
    assert payload["lastAlgorithmMetrics"]["pprCacheStatus"] == "disabled"
    assert payload["lastAlgorithmMetrics"]["reachableEdgesCacheStatus"] == "disabled"
    assert payload["lastAlgorithmMetrics"]["pathCandidateCount"] >= 1
    assert payload["lastTypedPathTrace"]["step"] == "typed-paths"
