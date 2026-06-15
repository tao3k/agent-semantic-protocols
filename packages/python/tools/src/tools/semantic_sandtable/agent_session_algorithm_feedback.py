"""Build graph-turbo algorithm feedback from agent-session reports."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any

from asp_graph_turbo.calibration import profile_calibration_from_feedback

from .utils import dict_value, list_value, require_str

_SCHEMA_ID = "agent.semantic-protocols.semantic-graph-turbo-feedback"
_PROTOCOL_ID = "agent.semantic-protocols.semantic-fact-frontier-feedback"
_REASON_RE = re.compile("[^a-z0-9_-]+")


def build_graph_turbo_algorithm_feedback(
    improvement_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
    *,
    source_path: str | None = None,
) -> dict[str, Any]:
    """Project analyzer improvement points into graph-turbo feedback receipts."""

    candidates = {
        require_str(candidate, "id", ""): candidate
        for candidate in list_value(graph_turbo_feedback.get("candidates"))
        if isinstance(candidate, dict)
    }
    nodes = []
    for index, point in enumerate(list_value(improvement_report.get("improvementPoints"))):
        if isinstance(point, dict) and point.get("category") == "graph-turbo":
            nodes.extend(_nodes_for_point(point, candidates, index))
    success_count = sum(1 for node in nodes if node["fields"]["effect"] == "boost")
    penalty_count = sum(1 for node in nodes if node["fields"]["effect"] == "penalty")
    return {
        "schemaId": _SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": _PROTOCOL_ID,
        "protocolVersion": "1",
        "packetKind": "graph-turbo-feedback",
        "source": "agent-session-analyzer",
        "sourcePath": source_path,
        "graph": {"nodes": nodes, "edges": []},
        "metrics": {
            "receiptNodeCount": len(nodes),
            "receiptEdgeCount": 0,
            "successCount": success_count,
            "penaltyCount": penalty_count,
        },
    }


def write_graph_turbo_algorithm_feedback(
    improvement_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
    output_path: Path,
    *,
    source_path: str | None = None,
) -> dict[str, Any]:
    packet = build_graph_turbo_algorithm_feedback(
        improvement_report,
        graph_turbo_feedback,
        source_path=source_path,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as handle:
        json.dump(packet, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return packet


def build_graph_turbo_calibration_proposal(
    algorithm_feedback: dict[str, Any],
    *,
    profile: str = "owner-query",
    request_packet: dict[str, Any] | None = None,
) -> dict[str, object]:
    """Convert analyzer feedback receipts into profile calibration deltas."""

    return profile_calibration_from_feedback(
        [algorithm_feedback],
        request_packet or {"graph": {"nodes": [], "edges": []}},
        profile=profile,
    )


def write_graph_turbo_calibration_proposal(
    algorithm_feedback: dict[str, Any],
    output_path: Path,
    *,
    profile: str = "owner-query",
    request_packet: dict[str, Any] | None = None,
) -> dict[str, object]:
    packet = build_graph_turbo_calibration_proposal(
        algorithm_feedback,
        profile=profile,
        request_packet=request_packet,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as handle:
        json.dump(packet, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return packet


def _nodes_for_point(
    point: dict[str, Any],
    candidates: dict[str, dict[str, Any]],
    index: int,
) -> list[dict[str, Any]]:
    candidate_ids = [str(item) for item in list_value(point.get("sourceCandidateIds"))]
    if not candidate_ids:
        return []
    candidate = candidates.get(candidate_ids[0], {})
    selector = _selector_for_point(point, candidate)
    reason = _reason_for_point(point, candidate)
    return [
        _receipt_node(
            point=point,
            selector=selector,
            reason=reason,
            effect=_effect_for_candidate(candidate),
            scope=_scope_for_candidate(candidate),
            target_kinds=_target_kinds_for_candidate(candidate),
            propagate_relations=_propagate_relations_for_candidate(candidate),
            propagation_factor=_propagation_factor_for_candidate(candidate),
            index=index,
        )
    ]


def _receipt_node(
    *,
    point: dict[str, Any],
    selector: str,
    reason: str,
    effect: str,
    scope: str,
    target_kinds: list[str],
    propagate_relations: list[str],
    propagation_factor: float,
    index: int,
) -> dict[str, Any]:
    point_id = require_str(point, "id", f"point-{index}")
    fields: dict[str, Any] = {
        "receiptKind": "frontier-success" if effect == "boost" else "frontier-waste",
        "effect": effect,
        "reason": reason,
        "selector": selector,
        "scope": scope,
        "scoreDelta": _score_delta(effect),
        "sourceScenario": require_str(point, "category", "graph-turbo"),
        "sourceStep": point_id,
    }
    if target_kinds:
        fields["targetKinds"] = target_kinds
    if scope == "relation-neighborhood":
        fields["propagateRelations"] = propagate_relations
        fields["propagationFactor"] = propagation_factor
    return {
        "id": "receipt:" + _stable_id(point_id, selector, reason),
        "kind": "receipt",
        "role": "frontier-feedback",
        "value": f"{effect}:{reason}:{selector}",
        "fields": fields,
    }


def _selector_for_point(point: dict[str, Any], candidate: dict[str, Any]) -> str:
    for key in ("matchedSelectors", "packetNodeIds", "repeatedQueries"):
        values = list_value(candidate.get(key))
        if values:
            return str(values[0])
    evidence_refs = list_value(point.get("evidenceRefs"))
    if evidence_refs:
        return str(evidence_refs[0])
    return require_str(point, "id", "graph-turbo")


def _reason_for_point(point: dict[str, Any], candidate: dict[str, Any]) -> str:
    reason = require_str(candidate, "kind", require_str(point, "category", "graph-turbo"))
    reason = _REASON_RE.sub("-", reason.lower()).strip("-")
    return reason or "graph-turbo"


def _effect_for_candidate(candidate: dict[str, Any]) -> str:
    kind = require_str(candidate, "kind", "")
    return (
        "boost"
        if kind
        in {
            "under-ranked-selector",
            "profile-rank-change",
            "path-intent-lost",
            "finder-path-ignored",
        }
        else "penalty"
    )


def _scope_for_candidate(candidate: dict[str, Any]) -> str:
    kind = require_str(candidate, "kind", "")
    if kind in {
        "under-ranked-selector",
        "repeated-query-group",
        "path-intent-lost",
        "finder-path-ignored",
    }:
        return "exact-selector"
    return "relation-neighborhood"


def _target_kinds_for_candidate(candidate: dict[str, Any]) -> list[str]:
    kind = require_str(candidate, "kind", "")
    return {
        "missing-fact": ["item", "owner", "dependency"],
        "under-ranked-selector": ["item", "hot", "owner"],
        "repeated-query-group": ["query"],
        "unclear-next-action": ["query", "owner"],
        "profile-rank-change": ["item", "owner", "dependency", "test"],
        "seed-plan-quality": ["query", "owner"],
        "path-intent-lost": ["owner", "item"],
        "finder-path-ignored": ["owner", "item", "hot"],
        "search-flow-drift": ["query", "owner"],
    }.get(kind, ["item", "owner"])


def _propagate_relations_for_candidate(candidate: dict[str, Any]) -> list[str]:
    kind = require_str(candidate, "kind", "")
    return {
        "missing-fact": ["contains", "covers"],
        "unclear-next-action": ["matches", "selects"],
        "profile-rank-change": ["contains", "imports", "covers"],
        "seed-plan-quality": ["matches", "contains", "selects"],
        "search-flow-drift": ["matches", "selects"],
    }.get(kind, ["contains"])


def _propagation_factor_for_candidate(candidate: dict[str, Any]) -> float:
    confidence = dict_value(candidate).get("confidence")
    if isinstance(confidence, int | float):
        return max(0.1, min(float(confidence), 1.0))
    return 0.5


def _score_delta(effect: str) -> float:
    return 0.65 if effect == "boost" else -0.65


def _stable_id(*parts: str) -> str:
    digest = hashlib.sha256("\0".join(parts).encode("utf-8")).hexdigest()
    return digest[:16]
