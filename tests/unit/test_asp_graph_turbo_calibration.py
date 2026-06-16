"""Profile-level graph-turbo calibration tests."""

from __future__ import annotations

from asp_graph_turbo.calibration import (
    apply_profile_calibrations,
    profile_calibration_from_feedback,
)
from asp_graph_turbo.profiles import resolve_profile

from ._asp_graph_turbo_common import (
    _GRAPH_TURBO_CALIBRATION_SCHEMA,
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


def test_calibration_boosts_profile_kind_bonus_from_success_feedback() -> None:
    request = _calibration_request()
    feedback = _feedback_packet(
        _receipt_node(
            "receipt:field-success",
            "frontier-success",
            "boost",
            "frontier-success",
            "src/good.py:10:20",
            3.0,
            target_kinds=["field"],
        )
    )

    calibration = profile_calibration_from_feedback(
        [feedback],
        request,
        profile="owner-query",
    )

    field_delta = _entry_by(calibration["kindDeltas"], "kind", "field")
    assert field_delta["scoreDelta"] == 0.3
    assert field_delta["receiptCount"] == 1
    assert field_delta["reasons"] == ["frontier-success"]
    assert calibration["guardrails"]["queryFirstStage"]["seedPrior"]["metric"] == (
        "querySeedPriorCount"
    )
    assert (
        list(
            schema_validator_for(_GRAPH_TURBO_CALIBRATION_SCHEMA).iter_errors(
                calibration
            )
        )
        == []
    )

    baseline = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(request),
            profile="owner-query",
            seeds=["q:feature"],
            limit=3,
            cache_enabled=False,
        )
    )
    adjusted_profile = apply_profile_calibrations(
        resolve_profile("owner-query"),
        [calibration],
    )
    adjusted = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(request),
            profile=adjusted_profile,
            seeds=["q:feature"],
            limit=3,
            cache_enabled=False,
        )
    )

    assert baseline["rank"].index("item:bad") < baseline["rank"].index("field:good")
    assert adjusted["rank"].index("field:good") < adjusted["rank"].index("item:bad")
    reasons = {
        explanation["nodeId"]: explanation["reasons"]
        for explanation in adjusted["rankExplanations"]
    }
    assert "kind-bonus:+0.70" in reasons["field:good"]
    selected = _profile_compatibility(adjusted, "owner-query")
    assert selected["kindBonus"]["field"] == 0.7
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(adjusted)) == []


def test_calibration_relation_delta_changes_profile_matrix_channel() -> None:
    request = _relation_request()
    feedback = _feedback_packet(
        _receipt_node(
            "receipt:collection-neighborhood",
            "frontier-success",
            "boost",
            "frontier-success",
            "src/good.py:10:20",
            2.0,
            target_kinds=["field"],
            scope="relation-neighborhood",
            propagate_relations=["collection_of"],
            propagate_kinds=["collection"],
            propagation_factor=0.5,
        )
    )
    calibration = profile_calibration_from_feedback(
        [feedback],
        request,
        profile="owner-query",
    )

    relation_delta = _entry_by(
        calibration["relationDeltas"],
        "relation",
        "collection_of",
    )
    assert relation_delta["weightMultiplierDelta"] == 0.08
    assert relation_delta["receiptCount"] == 1
    assert relation_delta["reasons"] == ["frontier-success:collection_of"]

    baseline = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(request),
            profile="owner-query",
            seeds=["q:feature"],
            limit=4,
            cache_enabled=False,
        )
    )
    adjusted = result_to_packet(
        rank_frontier(
            TypedGraph.from_packet(request),
            profile=apply_profile_calibrations(
                resolve_profile("owner-query"),
                [calibration],
            ),
            seeds=["q:feature"],
            limit=4,
            cache_enabled=False,
        )
    )

    baseline_channel = _profile_channel(baseline, "owner-query", "collection_of")
    adjusted_channel = _profile_channel(adjusted, "owner-query", "collection_of")
    assert adjusted_channel["weightMass"] > baseline_channel["weightMass"]
    assert (
        adjusted_channel["reachableWeightMass"]
        > baseline_channel["reachableWeightMass"]
    )
    selected = _profile_compatibility(adjusted, "owner-query")
    assert selected["relationWeightMultiplier"]["collection_of"] == 1.08
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(adjusted)) == []


def test_calibration_cli_builds_packet_and_rank_cli_consumes_it(tmp_path: Path) -> None:
    request_path = tmp_path / "request.json"
    feedback_path = tmp_path / "feedback.json"
    calibration_path = tmp_path / "calibration.json"
    request_path.write_text(json.dumps(_relation_request()), encoding="utf-8")
    feedback_path.write_text(
        json.dumps(
            _feedback_packet(
                _receipt_node(
                    "receipt:collection-neighborhood",
                    "frontier-success",
                    "boost",
                    "frontier-success",
                    "src/good.py:10:20",
                    2.0,
                    target_kinds=["field"],
                    scope="relation-neighborhood",
                    propagate_relations=["collection_of"],
                    propagate_kinds=["collection"],
                    propagation_factor=0.5,
                )
            )
        ),
        encoding="utf-8",
    )

    calibration_result = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "calibrate",
            str(feedback_path),
            str(request_path),
            "--profile",
            "owner-query",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    calibration_path.write_text(calibration_result.stdout, encoding="utf-8")
    calibration = json.loads(calibration_result.stdout)

    assert calibration["schemaId"] == (
        "agent.semantic-protocols.semantic-graph-turbo-calibration"
    )
    assert (
        list(
            schema_validator_for(_GRAPH_TURBO_CALIBRATION_SCHEMA).iter_errors(
                calibration
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
            "--calibration",
            str(calibration_path),
            "--format",
            "json",
            str(request_path),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(ranked.stdout)

    selected = _profile_compatibility(payload, "owner-query")
    assert selected["relationWeightMultiplier"]["collection_of"] == 1.08
    assert _profile_channel(payload, "owner-query", "collection_of")["weightMass"] > 1.3
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(payload)) == []


def _calibration_request() -> dict[str, object]:
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
                    "id": "field:good",
                    "kind": "field",
                    "role": "member",
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
                    "weight": 1.25,
                },
            ],
            "edges": [
                {"source": "q:feature", "target": "field:good", "relation": "matches"},
                {"source": "q:feature", "target": "item:bad", "relation": "matches"},
            ],
        },
    }


def _relation_request() -> dict[str, object]:
    request = _calibration_request()
    graph = request["graph"]
    assert isinstance(graph, dict)
    nodes = graph["nodes"]
    edges = graph["edges"]
    assert isinstance(nodes, list)
    assert isinstance(edges, list)
    nodes.append(
        {
            "id": "collection:vec",
            "kind": "collection",
            "role": "family",
            "value": "Vec",
            "weight": 1.0,
        }
    )
    edges.append(
        {
            "source": "field:good",
            "target": "collection:vec",
            "relation": "collection_of",
        }
    )
    return request


def _feedback_packet(*nodes: dict[str, object]) -> dict[str, object]:
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


def _entry_by(entries: object, key: str, value: str) -> dict[str, object]:
    assert isinstance(entries, list)
    for entry in entries:
        assert isinstance(entry, dict)
        if entry.get(key) == value:
            return entry
    raise AssertionError(f"entry not found: {key}={value}")


def _profile_compatibility(
    packet: dict[str, object], profile: str
) -> dict[str, object]:
    return _entry_by(packet["profileCompatibility"], "profile", profile)


def _profile_channel(
    packet: dict[str, object], profile: str, relation: str
) -> dict[str, object]:
    matrix = _entry_by(packet["profileMatrices"], "profile", profile)
    return _entry_by(matrix["relationChannels"], "relation", relation)
