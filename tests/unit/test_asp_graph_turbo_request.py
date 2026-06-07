"""Focused ASP graph turbo tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    _GRAPH_TURBO_FIXTURE,
    _GRAPH_TURBO_REQUEST_SCHEMA,
    _GRAPH_TURBO_SCHEMA,
    Path,
    TypedGraph,
    json,
    rank_frontier,
    result_to_packet,
    sample_packet,
    sample_request,
    schema_validator_for,
    subprocess,
    sys,
)


def test_request_fixture_is_schema_owned_algorithm_input() -> None:
    packet = json.loads(_GRAPH_TURBO_FIXTURE.read_text(encoding="utf-8"))
    errors = list(schema_validator_for(_GRAPH_TURBO_REQUEST_SCHEMA).iter_errors(packet))

    assert errors == []
    assert packet["packetKind"] == "graph-turbo-request"
    assert packet["algorithm"] == "typed-ppr-diverse"
    assert packet["kindBudgets"]["owner"] == 2
    assert packet["windowMerge"]["maxGapLines"] == 8


def test_request_schema_accepts_read_memory_seen_selectors() -> None:
    packet = sample_request()
    packet["readMemory"] = {"seenSelectors": ["src/cli.py:10:20"]}

    errors = list(schema_validator_for(_GRAPH_TURBO_REQUEST_SCHEMA).iter_errors(packet))

    assert errors == []


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
        "failure-frontier",
        "field-impact",
        "type-impact",
        "collection-impact",
        "failure-evidence",
        "test-selection",
        "affected",
    ]
    assert packet["profileCompatibility"][0]["profile"] == "owner-query"
    assert packet["profileCompatibility"][0]["compatible"] is True
    assert {
        "sourceKind": "item",
        "targetKind": "hot",
    } in packet["profileCompatibility"][0]["allowedTransitions"]
    assert {
        "sourceKind": "field",
        "targetKind": "hot",
    } in packet["profileCompatibility"][0]["allowedTransitions"]
    assert packet["profileCompatibility"][0]["kindBonus"]["field"] == 0.4
    assert packet["profileCompatibility"][0]["kindBonus"]["hot"] == 0.35
    assert packet["mergedWindows"] == []
    assert packet["sourceSinkFrontier"]["sourceIds"] == ["q:parser", "owner:cli"]
    assert any(
        path["pathKind"] == "constrained-shortest" for path in packet["typedPaths"]
    )
    assert packet["flowLite"]["rankedPathIds"][0] == packet["typedPaths"][0]["id"]
    assert packet["packetFingerprint"].startswith("sha256:")
    assert packet["graphCache"]["backend"] == "scipy-csr"
    assert packet["edges"][0]["weight"] > 0
    assert packet["algorithmTrace"][2]["step"] == "profile-policy"
    assert packet["algorithmTrace"][3]["step"] == "typed-ppr"
    assert any(
        explanation["nodeId"] == "q:parser"
        for explanation in packet["rankExplanations"]
    )
    assert packet["algorithmMetrics"]["pathCount"] == len(packet["typedPaths"])
    assert packet["rank"] == [node["id"] for node in packet["rankedNodes"]]
    assert any(entry["action"] == "fzf" for entry in packet["frontier"])
    assert packet["omit"] == ["code", "full-score-vector", "full-graph"]
    assert packet["avoid"] == [
        "raw-read",
        "repeat-owner",
        "broad-fzf",
        "manual-window-scan",
    ]


def test_tools_graph_turbo_cli_uses_request_packet_defaults(tmp_path: Path) -> None:
    packet = tmp_path / "graph.json"
    packet.write_text(
        json.dumps(sample_request(profile="query-deps", budget=4)), encoding="utf-8"
    )

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


def test_tools_graph_turbo_cli_applies_read_memory_suppression(tmp_path: Path) -> None:
    packet = tmp_path / "graph.json"
    request = sample_request()
    request["readMemory"] = {"seenSelectors": ["src/cli.py:10:20"]}
    packet.write_text(json.dumps(request), encoding="utf-8")

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

    assert "item:collect" not in payload["rank"]
    assert payload["algorithmMetrics"]["readMemorySuppressedCount"] == 1
    assert "seen-selector" in payload["avoid"]
