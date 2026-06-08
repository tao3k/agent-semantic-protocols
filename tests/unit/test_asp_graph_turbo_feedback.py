"""Receipt-driven graph-turbo feedback tests."""

from __future__ import annotations

from asp_graph_turbo.feedback import (
    feedback_packet_from_sandtable,
    merge_feedback_into_packet,
)

from ._asp_graph_turbo_common import (
    _GRAPH_TURBO_FEEDBACK_SCHEMA,
    _GRAPH_TURBO_SCHEMA,
    Path,
    TypedGraph,
    json,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
    subprocess,
    sys,
)


def test_feedback_receipts_boost_success_and_penalize_waste() -> None:
    request = _feedback_request()
    baseline = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(request),
            profile="owner-query",
            seeds=["q:feature"],
            limit=4,
            cache_enabled=False,
        )
    )
    assert baseline["rank"].index("item:bad") < baseline["rank"].index("item:good")

    feedback = _feedback_packet()
    adjusted = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(merge_feedback_into_packet(request, [feedback])),
            profile="owner-query",
            seeds=["q:feature"],
            limit=4,
            cache_enabled=False,
        )
    )

    assert adjusted["rank"].index("item:good") < adjusted["rank"].index("item:bad")
    assert adjusted["algorithmMetrics"]["receiptBoostCount"] == 1
    assert adjusted["algorithmMetrics"]["receiptPenaltyCount"] == 1
    assert adjusted["receiptAdjustments"] == [
        {
            "nodeId": "item:good",
            "effect": "boost",
            "scoreDelta": 3.0,
            "reason": "frontier-success",
        },
        {
            "nodeId": "item:bad",
            "effect": "penalty",
            "scoreDelta": -3.0,
            "reason": "frontier-waste",
        },
    ]
    reasons = {
        explanation["nodeId"]: explanation["reasons"]
        for explanation in adjusted["rankExplanations"]
    }
    assert "receipt-boost:+3.00:frontier-success" in reasons["item:good"]
    assert "receipt-penalty:-3.00:frontier-waste" in reasons["item:bad"]
    assert adjusted["profileMatrices"][0]["profile"] == "owner-query"
    assert adjusted["profileMatrices"][0]["relationMatrixCount"] > 0
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(adjusted)) == []


def test_feedback_policy_can_target_exact_kind_without_hot_shadow() -> None:
    request = _feedback_request_with_shared_selector()
    feedback = _policy_feedback_packet(
        _receipt_node(
            "receipt:field-only",
            "frontier-success",
            "boost",
            "frontier-success",
            "src/good.py:10:20",
            2.0,
            target_kinds=["item"],
        )
    )

    adjusted = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(merge_feedback_into_packet(request, [feedback])),
            profile="owner-query",
            seeds=["q:feature"],
            limit=8,
            cache_enabled=False,
        )
    )

    assert adjusted["receiptAdjustments"] == [
        {
            "nodeId": "item:good",
            "effect": "boost",
            "scoreDelta": 2.0,
            "reason": "frontier-success",
        }
    ]


def test_feedback_policy_can_propagate_to_relation_neighbors() -> None:
    request = _feedback_request_with_shared_selector()
    feedback = _policy_feedback_packet(
        _receipt_node(
            "receipt:collection",
            "frontier-success",
            "boost",
            "frontier-success",
            "src/good.py:10:20",
            2.0,
            target_kinds=["item"],
            scope="relation-neighborhood",
            propagate_relations=["collection_of"],
            propagate_kinds=["collection"],
            propagation_factor=0.5,
        )
    )

    adjusted = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(merge_feedback_into_packet(request, [feedback])),
            profile="owner-query",
            seeds=["q:feature"],
            limit=8,
            cache_enabled=False,
        )
    )

    assert {
        "nodeId": "collection:vec",
        "effect": "boost",
        "scoreDelta": 1.0,
        "reason": "frontier-success:collection_of",
    } in adjusted["receiptAdjustments"]
    reasons = {
        explanation["nodeId"]: explanation["reasons"]
        for explanation in adjusted["rankExplanations"]
    }
    assert (
        "receipt-boost:+1.00:frontier-success:collection_of"
        in reasons["collection:vec"]
    )


def test_feedback_policy_accumulates_multiple_receipts() -> None:
    request = _feedback_request()
    feedback = _policy_feedback_packet(
        _receipt_node(
            "receipt:good-1",
            "frontier-success",
            "boost",
            "frontier-success",
            "src/good.py:10:20",
            0.6,
        ),
        _receipt_node(
            "receipt:good-2",
            "frontier-success",
            "boost",
            "frontier-success",
            "src/good.py:10:20",
            0.7,
        ),
    )

    adjusted = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(merge_feedback_into_packet(request, [feedback])),
            profile="owner-query",
            seeds=["q:feature"],
            limit=4,
            cache_enabled=False,
        )
    )

    assert adjusted["algorithmMetrics"]["receiptBoostCount"] == 2
    assert adjusted["scores"]["item:good"] > adjusted["scores"]["item:bad"]
    reasons = {
        explanation["nodeId"]: explanation["reasons"]
        for explanation in adjusted["rankExplanations"]
    }
    assert "receipt-boost:+0.60:frontier-success" in reasons["item:good"]
    assert "receipt-boost:+0.70:frontier-success" in reasons["item:good"]


def test_feedback_cli_builds_packet_and_rank_cli_consumes_it(tmp_path: Path) -> None:
    report = tmp_path / "sandtable.json"
    feedback = tmp_path / "feedback.json"
    request = tmp_path / "request.json"
    sandtable_report = _sandtable_report()
    direct_packet = feedback_packet_from_sandtable(sandtable_report)
    assert direct_packet["metrics"]["successCount"] == 1
    report.write_text(json.dumps(sandtable_report), encoding="utf-8")
    request.write_text(json.dumps(_feedback_request()), encoding="utf-8")

    feedback_result = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "feedback",
            str(report),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    feedback.write_text(feedback_result.stdout, encoding="utf-8")
    feedback_packet = json.loads(feedback_result.stdout)

    assert feedback_packet["schemaId"] == (
        "agent.semantic-protocols.semantic-graph-turbo-feedback"
    )
    assert feedback_packet["metrics"]["successCount"] == 1
    assert feedback_packet["metrics"]["penaltyCount"] == 1
    assert (
        list(
            schema_validator_for(_GRAPH_TURBO_FEEDBACK_SCHEMA).iter_errors(
                feedback_packet
            )
        )
        == []
    )

    ranked = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "rank",
            "--feedback",
            str(feedback),
            "--format",
            "json",
            str(request),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(ranked.stdout)

    assert payload["rank"].index("item:good") < payload["rank"].index("item:bad")
    assert payload["algorithmMetrics"]["receiptBoostCount"] == 1
    assert payload["algorithmMetrics"]["receiptPenaltyCount"] == 1
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(payload)) == []


def _feedback_request() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["q:feature"],
        "budget": 4,
        "cache": {"enabled": False},
        "graph": {
            "nodes": [
                {
                    "id": "q:feature",
                    "kind": "query",
                    "role": "term",
                    "value": "feature",
                },
                {
                    "id": "item:good",
                    "kind": "item",
                    "role": "fn",
                    "value": "feature_good",
                    "locator": "src/good.py:10:20",
                    "ownerPath": "src/good.py",
                    "weight": 1.0,
                },
                {
                    "id": "item:bad",
                    "kind": "item",
                    "role": "fn",
                    "value": "feature_bad",
                    "locator": "src/bad.py:10:20",
                    "ownerPath": "src/bad.py",
                    "weight": 2.0,
                },
            ],
            "edges": [
                {"source": "q:feature", "target": "item:good", "relation": "matches"},
                {"source": "q:feature", "target": "item:bad", "relation": "matches"},
            ],
        },
    }


def _feedback_request_with_shared_selector() -> dict[str, object]:
    request = _feedback_request()
    graph = request["graph"]
    assert isinstance(graph, dict)
    nodes = graph["nodes"]
    edges = graph["edges"]
    assert isinstance(nodes, list)
    assert isinstance(edges, list)
    nodes.extend(
        [
            {
                "id": "hot:good",
                "kind": "hot",
                "role": "range",
                "value": "feature_good_hot",
                "locator": "src/good.py:10:20",
                "ownerPath": "src/good.py",
                "weight": 1.0,
            },
            {
                "id": "collection:vec",
                "kind": "collection",
                "role": "family",
                "value": "Vec",
                "weight": 1.0,
            },
        ]
    )
    edges.extend(
        [
            {"source": "item:good", "target": "hot:good", "relation": "contains"},
            {
                "source": "item:good",
                "target": "collection:vec",
                "relation": "collection_of",
            },
            {
                "source": "q:feature",
                "target": "collection:vec",
                "relation": "matches",
            },
        ]
    )
    return request


def _feedback_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-feedback",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-fact-frontier-feedback",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-feedback",
        "source": "unit-test",
        "sourcePath": None,
        "graph": {
            "nodes": [
                _receipt_node(
                    "receipt:good",
                    "frontier-success",
                    "boost",
                    "frontier-success",
                    "src/good.py:10:20",
                    3.0,
                ),
                _receipt_node(
                    "receipt:bad",
                    "frontier-waste",
                    "penalty",
                    "frontier-waste",
                    "src/bad.py:10:20",
                    -3.0,
                ),
            ],
            "edges": [],
        },
        "metrics": {
            "receiptNodeCount": 2,
            "receiptEdgeCount": 0,
            "successCount": 1,
            "penaltyCount": 1,
        },
    }


def _policy_feedback_packet(*nodes: dict[str, object]) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-feedback",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-fact-frontier-feedback",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-feedback",
        "source": "unit-test",
        "sourcePath": None,
        "graph": {"nodes": list(nodes), "edges": []},
        "metrics": {
            "receiptNodeCount": len(nodes),
            "receiptEdgeCount": 0,
            "successCount": sum(
                1
                for node in nodes
                if isinstance(node.get("fields"), dict)
                and node["fields"].get("effect") == "boost"
            ),
            "penaltyCount": sum(
                1
                for node in nodes
                if isinstance(node.get("fields"), dict)
                and node["fields"].get("effect") == "penalty"
            ),
        },
    }


def _sandtable_report() -> dict[str, object]:
    return {
        "scenarios": [
            {
                "id": "python.feedback-flow",
                "steps": [
                    {
                        "id": "agent-answer",
                        "status": "pass",
                        "observations": {
                            "finalAnswer": {
                                "present": True,
                                "afterLastToolUse": True,
                            },
                            "pipeFlow": {
                                "commands": [
                                    "asp python search pipe feature --view seeds .",
                                    "asp python query --selector src/good.py:10:20 --code .",
                                    "asp python query --selector src/bad.py:10:20 --code .",
                                ]
                            },
                        },
                    }
                ],
            }
        ]
    }


def _receipt_node(
    node_id: str,
    receipt_kind: str,
    effect: str,
    reason: str,
    selector: str,
    score_delta: float,
    *,
    scope: str = "exact-selector",
    target_kinds: list[str] | None = None,
    propagate_relations: list[str] | None = None,
    propagate_kinds: list[str] | None = None,
    propagation_factor: float | None = None,
) -> dict[str, object]:
    fields: dict[str, object] = {
        "receiptKind": receipt_kind,
        "effect": effect,
        "reason": reason,
        "selector": selector,
        "scope": scope,
        "scoreDelta": score_delta,
    }
    if target_kinds is not None:
        fields["targetKinds"] = target_kinds
    if propagate_relations is not None:
        fields["propagateRelations"] = propagate_relations
    if propagate_kinds is not None:
        fields["propagateKinds"] = propagate_kinds
    if propagation_factor is not None:
        fields["propagationFactor"] = propagation_factor
    return {
        "id": node_id,
        "kind": "receipt",
        "role": "frontier-feedback",
        "value": f"{effect}:{selector}",
        "fields": fields,
    }
