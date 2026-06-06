"""Tests for the ASP graph turbo Python package."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from asp_graph_turbo import TypedGraph, rank_frontier, render_compact, result_to_packet
from unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[2]
_GRAPH_TURBO_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-result.v1.schema.json"
)
_GRAPH_TURBO_REQUEST_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-request.v1.schema.json"
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
                "symbol": "collect_actions",
            },
            {
                "id": "hot:command",
                "kind": "hot",
                "role": "call",
                "value": "command_intent",
                "owner": "src/cli.py",
                "symbol": "command_intent",
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


def sample_request(*, profile: str = "owner-query", budget: int = 8) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "profile": profile,
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["q:parser", "owner:cli"],
        "budget": budget,
        "kindBudgets": {"owner": 1, "dependency": 1, "test": 1},
        "windowMerge": {"enabled": True, "maxGapLines": 8},
        "pathBudget": 4,
        "pathMaxHops": 4,
        "cache": {"enabled": True},
        "graph": sample_packet(),
    }


def test_request_fixture_is_schema_owned_algorithm_input() -> None:
    packet = json.loads(_GRAPH_TURBO_FIXTURE.read_text(encoding="utf-8"))
    errors = list(schema_validator_for(_GRAPH_TURBO_REQUEST_SCHEMA).iter_errors(packet))

    assert errors == []
    assert packet["packetKind"] == "graph-turbo-request"
    assert packet["algorithm"] == "typed-ppr-diverse"
    assert packet["kindBudgets"]["owner"] == 2
    assert packet["windowMerge"]["maxGapLines"] == 8


def test_owner_query_profile_masks_dependency_edges() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(graph, profile="owner-query", seeds=["q:parser", "owner:cli"])

    ranked = [node.id for node in result.ranked_nodes]

    assert "owner:cli" in ranked
    assert "item:collect" in ranked
    assert "hot:command" in ranked
    assert "test:cli" in ranked
    assert "dep:jsonschema" not in ranked
    assert ("dependency", "deps") not in [
        (entry.node.kind, entry.action) for entry in result.frontier
    ]


def test_query_deps_profile_can_cross_dependency_edges() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(graph, profile="query-deps", seeds=["q:parser"])

    ranked = [node.id for node in result.ranked_nodes]

    assert "dep:jsonschema" in ranked
    assert any(entry.node.id == "dep:jsonschema" and entry.action == "deps" for entry in result.frontier)


def test_compact_render_uses_asp_graph_frontier_contract() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(graph, profile="owner-query", seeds=["q:parser", "owner:cli"])

    compact = render_compact(result)

    assert compact.startswith(
        "[graph-frontier] profile=owner-query alg=typed-ppr-diverse seed=Q,O budget=8\n"
    )
    assert "legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next" in compact
    assert "aliases=G:graph" in compact
    assert "Q=query:term(parser)!fzf" in compact
    assert "I=item:fn(collect_actions)!code" in compact
    assert "H=hot:call(command_intent)!code" in compact
    assert "G>{" in compact and "Q:matches" in compact and "O:selects" in compact
    assert "Q>{I:matches}" in compact
    assert "O>{" in compact and "T:covers" in compact
    assert "\nrank=" in compact
    assert "\nfrontier=" in compact
    assert "\nscores=" in compact
    assert "Q:" in compact and "O:" in compact and "T:" in compact
    assert "\nprofiles=owner-query,query-deps,owner-tests,prime,read-frontier\n" in compact
    assert "\nomit=code,full-score-vector,full-graph\n" in compact
    assert "\navoid=raw-read,repeat-owner,broad-fzf\n" in compact
    assert "[graph-turbo]" not in compact
    assert "aliases:" not in compact


def test_result_packet_is_schema_owned_ranking_evidence() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parser", "owner:cli", "q:parser"],
    )
    packet = result_to_packet(result)
    errors = list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet))

    assert errors == []
    assert packet["schemaId"] == "agent.semantic-protocols.semantic-graph-turbo-result"
    assert packet["algorithm"] == "typed-ppr-diverse"
    assert packet["seedIds"] == ["q:parser", "owner:cli"]
    assert packet["budget"] == 8
    assert packet["kindBudgets"] == {}
    assert packet["profiles"] == [
        "owner-query",
        "query-deps",
        "owner-tests",
        "prime",
        "read-frontier",
    ]
    assert packet["profileCompatibility"][0]["profile"] == "owner-query"
    assert packet["profileCompatibility"][0]["compatible"] is True
    assert packet["profileCompatibility"][0]["allowedTransitions"][0] == {
        "sourceKind": "item",
        "targetKind": "hot",
    }
    assert packet["profileCompatibility"][0]["kindBonus"]["hot"] == 0.3
    assert packet["mergedWindows"] == []
    assert packet["sourceSinkFrontier"]["sourceIds"] == ["q:parser", "owner:cli"]
    assert any(path["pathKind"] == "constrained-shortest" for path in packet["typedPaths"])
    assert packet["flowLite"]["rankedPathIds"][0] == packet["typedPaths"][0]["id"]
    assert packet["packetFingerprint"].startswith("sha256:")
    assert packet["graphCache"]["backend"] == "scipy-csr"
    assert packet["edges"][0]["weight"] > 0
    assert packet["algorithmTrace"][2]["step"] == "profile-policy"
    assert packet["algorithmTrace"][3]["step"] == "typed-ppr"
    assert any(
        explanation["nodeId"] == "q:parser" for explanation in packet["rankExplanations"]
    )
    assert packet["algorithmMetrics"]["pathCount"] == len(packet["typedPaths"])
    assert packet["rank"] == [node["id"] for node in packet["rankedNodes"]]
    assert any(entry["action"] == "fzf" for entry in packet["frontier"])
    assert packet["omit"] == ["code", "full-score-vector", "full-graph"]
    assert packet["avoid"] == ["raw-read", "repeat-owner", "broad-fzf"]


def test_tools_graph_turbo_cli_uses_request_packet_defaults(tmp_path: Path) -> None:
    packet = tmp_path / "graph.json"
    packet.write_text(json.dumps(sample_request(profile="query-deps", budget=4)), encoding="utf-8")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "tools",
            "graph",
            "turbo",
            str(packet),
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(completed.stdout)
    errors = list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(payload))

    assert errors == []
    assert payload["packetKind"] == "graph-turbo-result"
    assert payload["profile"] == "query-deps"
    assert payload["budget"] == 4
    assert payload["kindBudgets"] == {"owner": 1, "dependency": 1, "test": 1}
    assert payload["algorithmMetrics"]["cacheStatus"] in {"hit", "miss"}
    assert payload["sourceSinkFrontier"]["sourceIds"] == ["q:parser", "owner:cli"]
    assert "dep:jsonschema" in payload["rank"]
    assert any(entry["action"] == "deps" for entry in payload["frontier"])
