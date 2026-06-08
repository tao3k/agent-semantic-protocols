"""Rank command tests for the packaged ASP graph turbo CLI."""

from __future__ import annotations

import json
import subprocess
import sys

from unit.asp_graph_turbo_cli_support import (
    sample_graph_turbo_request,
    validate_shared_schema,
)


def test_graph_turbo_rank_compact_projects_algorithm_evidence(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")

    completed = subprocess.run(
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
    )

    stdout = completed.stdout
    assert stdout.startswith(
        "[graph-frontier] profile=owner-query alg=typed-ppr-diverse"
    )
    assert "\nscores=" in stdout
    assert "\npaths=P" in stdout
    assert "\ncache=" in stdout
    assert "\ntrace=" in stdout
    assert "typed-ppr:scipy-csr" in stdout
    assert "\nexplain=" in stdout
    assert "relation:matches" in stdout
    assert "\nmetrics=" in stdout


def test_graph_turbo_request_fixture_matches_shared_schema() -> None:
    validate_shared_schema(
        sample_graph_turbo_request(),
        "semantic-graph-turbo-request.v1.schema.json",
    )


def test_graph_turbo_rank_json_owns_trace_path_score_explanations(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "rank",
            str(packet_path),
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(completed.stdout)
    validate_shared_schema(payload, "semantic-graph-turbo-result.v1.schema.json")

    assert payload["schemaId"] == "agent.semantic-protocols.semantic-graph-turbo-result"
    assert payload["packetKind"] == "graph-turbo-result"
    assert payload["algorithm"] == "typed-ppr-diverse"
    assert payload["scores"]
    assert payload["typedPaths"][0]["rank"] == 1
    assert payload["graphCache"]["backend"] == "scipy-csr"
    assert payload["algorithmTrace"]
    assert any(step["step"] == "typed-ppr" for step in payload["algorithmTrace"])
    assert payload["rankExplanations"]
    assert payload["algorithmMetrics"]["pathCount"] >= 1
    assert payload["algorithmMetrics"]["pathBackend"] in {
        "python-bfs-small",
        "scipy-yen",
        "scipy-dijkstra",
        "python-bfs-fallback",
    }
    assert payload["algorithmMetrics"]["pathFallbackCount"] >= 0
    assert payload["algorithmMetrics"]["pathPairCount"] >= 1
    assert payload["algorithmMetrics"]["pathCandidateCount"] >= 1


def test_graph_turbo_rank_summary_json_preserves_frontier_without_full_packet(
    tmp_path,
) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")

    summary = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "rank",
            str(packet_path),
            "--format",
            "summary-json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    full = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "rank",
            str(packet_path),
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(summary.stdout)
    validate_shared_schema(payload, "semantic-graph-turbo-summary.v1.schema.json")

    assert len(summary.stdout) < len(full.stdout)
    assert payload["schemaId"] == "agent.semantic-protocols.semantic-graph-turbo-summary"
    assert payload["packetKind"] == "graph-turbo-summary"
    assert payload["sourcePacketKind"] == "graph-turbo-result"
    assert payload["frontier"][0]["selector"] == "src/lib.rs:1:1"
    assert payload["rankedNodes"][0]["score"] is not None
    assert payload["typedPaths"][0]["rank"] == 1
    assert payload["algorithmMetrics"]["pathCandidateCount"] >= 1
    assert "full-score-vector" in payload["projection"]["omitted"]
    assert "full-node-fields" in payload["projection"]["omitted"]
