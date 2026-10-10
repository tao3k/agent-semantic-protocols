# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Algorithm API tests; Runtime ranking is served through private gRPC."""

from __future__ import annotations

from asp_python_graphs import render_compact
from asp_python_graphs.algorithm import RankOptions, rank_packet
from asp_python_graphs.packet import result_to_packet
from asp_python_graphs.summary_packet import result_to_summary_packet

from unit.asp_graph_turbo_cli_support import (
    sample_graph_turbo_request,
    validate_shared_schema,
)


def test_graph_turbo_algorithm_compact_projects_algorithm_evidence() -> None:
    stdout = render_compact(
        rank_packet(sample_graph_turbo_request(), RankOptions())
    )
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


def test_graph_turbo_algorithm_json_owns_trace_path_score_explanations() -> None:
    payload = result_to_packet(
        rank_packet(sample_graph_turbo_request(), RankOptions())
    )
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


def test_graph_turbo_algorithm_summary_preserves_frontier_without_full_packet() -> None:
    result = rank_packet(sample_graph_turbo_request(), RankOptions())
    payload = result_to_summary_packet(result)
    full_payload = result_to_packet(result)

    validate_shared_schema(payload, "semantic-graph-turbo-summary.v1.schema.json")

    assert len(str(payload)) < len(str(full_payload))
    assert payload["schemaId"] == "agent.semantic-protocols.semantic-graph-turbo-summary"
    assert payload["packetKind"] == "graph-turbo-summary"
    assert payload["sourcePacketKind"] == "graph-turbo-result"
    assert any(
        entry["selector"] == "src/lib.rs:1:1" for entry in payload["rankedNodes"]
    )
    assert payload["rankedNodes"][0]["score"] is not None
    assert payload["typedPaths"][0]["rank"] == 1
    assert payload["algorithmMetrics"]["pathCandidateCount"] >= 1
    assert "full-score-vector" in payload["projection"]["omitted"]
    assert "full-node-fields" in payload["projection"]["omitted"]
