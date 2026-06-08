"""Profile-level calibration derived from graph-turbo feedback facts."""

from __future__ import annotations

import json
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping
from typing import Any

from .calibration_apply import apply_profile_calibrations as apply_profile_calibrations
from .feedback import merge_feedback_into_packet
from .model import Node, TypedGraph
from .selector import graph_turbo_selector_for_node

CALIBRATION_SCHEMA_ID = "agent.semantic-protocols.semantic-graph-turbo-calibration"
CALIBRATION_PROTOCOL_ID = "agent.semantic-protocols.semantic-fact-frontier-feedback"

_KIND_DELTA_FACTOR = 0.1
_RELATION_DELTA_FACTOR = 0.08


def profile_calibration_from_feedback(
    feedback_packets: Iterable[Mapping[str, Any]],
    request_packet: Mapping[str, Any],
    *,
    profile: str,
) -> dict[str, object]:
    """Build profile-level calibration deltas from feedback and a graph request."""

    feedback_tuple = tuple(feedback_packets)
    graph = TypedGraph.from_packet(
        merge_feedback_into_packet(request_packet, feedback_tuple)
    )
    kind_deltas: dict[str, float] = defaultdict(float)
    kind_receipts: Counter[str] = Counter()
    kind_reasons: dict[str, set[str]] = defaultdict(set)
    relation_deltas: dict[str, float] = defaultdict(float)
    relation_receipts: Counter[str] = Counter()
    relation_reasons: dict[str, set[str]] = defaultdict(set)
    receipt_count = 0
    for receipt in (node for node in graph.nodes.values() if node.kind == "receipt"):
        receipt_count += 1
        _apply_receipt_kind_delta(
            graph, receipt, kind_deltas, kind_receipts, kind_reasons
        )
        _apply_receipt_relation_delta(
            receipt, relation_deltas, relation_receipts, relation_reasons
        )

    return {
        "schemaId": CALIBRATION_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": CALIBRATION_PROTOCOL_ID,
        "protocolVersion": "1",
        "packetKind": "graph-turbo-calibration",
        "profile": profile,
        "source": "feedback",
        "kindDeltas": [
            {
                "kind": kind,
                "scoreDelta": round(delta, 6),
                "receiptCount": kind_receipts[kind],
                "reasons": sorted(kind_reasons[kind]),
            }
            for kind, delta in sorted(kind_deltas.items())
            if delta
        ],
        "relationDeltas": [
            {
                "relation": relation,
                "weightMultiplierDelta": round(delta, 6),
                "receiptCount": relation_receipts[relation],
                "reasons": sorted(relation_reasons[relation]),
            }
            for relation, delta in sorted(relation_deltas.items())
            if delta
        ],
        "metrics": {
            "feedbackPacketCount": len(feedback_tuple),
            "receiptCount": receipt_count,
            "kindDeltaCount": sum(1 for delta in kind_deltas.values() if delta),
            "relationDeltaCount": sum(1 for delta in relation_deltas.values() if delta),
        },
    }


def calibration_to_json(packet: Mapping[str, object]) -> str:
    return json.dumps(packet, sort_keys=True) + "\n"


def _apply_receipt_kind_delta(
    graph: TypedGraph,
    receipt: Node,
    kind_deltas: dict[str, float],
    kind_receipts: Counter[str],
    kind_reasons: dict[str, set[str]],
) -> None:
    effect = _effect(receipt)
    if effect is None:
        return
    for node in _matched_nodes(graph, receipt):
        kind_deltas[node.kind] += _score_delta(receipt) * _KIND_DELTA_FACTOR
        kind_receipts[node.kind] += 1
        kind_reasons[node.kind].add(_reason(receipt))


def _apply_receipt_relation_delta(
    receipt: Node,
    relation_deltas: dict[str, float],
    relation_receipts: Counter[str],
    relation_reasons: dict[str, set[str]],
) -> None:
    effect = _effect(receipt)
    if effect is None or _scope(receipt) != "relation-neighborhood":
        return
    relation_delta = (
        _score_delta(receipt) * _propagation_factor(receipt) * _RELATION_DELTA_FACTOR
    )
    for relation in _string_set_field(receipt, "propagateRelations"):
        relation_deltas[relation] += relation_delta
        relation_receipts[relation] += 1
        relation_reasons[relation].add(f"{_reason(receipt)}:{relation}")


def _matched_nodes(graph: TypedGraph, receipt: Node) -> tuple[Node, ...]:
    selector = _string_field(receipt, "selector")
    if selector is None:
        return ()
    target_kinds = _string_set_field(receipt, "targetKinds")
    matches = tuple(
        node
        for node in graph.nodes.values()
        if node.kind != "receipt"
        and graph_turbo_selector_for_node(node) == selector
        and (not target_kinds or node.kind in target_kinds)
    )
    parser_owned = tuple(
        node for node in matches if node.kind in {"field", "type", "collection"}
    )
    return parser_owned or matches


def _effect(receipt: Node) -> str | None:
    effect = _field(receipt, "effect")
    if effect in {"boost", "penalty"}:
        return str(effect)
    return None


def _score_delta(receipt: Node) -> float:
    value = _field(receipt, "scoreDelta")
    if isinstance(value, int | float):
        return float(value)
    return 0.0


def _reason(receipt: Node) -> str:
    reason = _field(receipt, "reason")
    return str(reason) if isinstance(reason, str) and reason else "feedback"


def _propagation_factor(receipt: Node) -> float:
    value = _field(receipt, "propagationFactor")
    return float(value) if isinstance(value, int | float) else 0.35


def _scope(receipt: Node) -> str:
    scope = _field(receipt, "scope")
    return str(scope) if isinstance(scope, str) and scope else "exact-selector"


def _string_field(receipt: Node, name: str) -> str | None:
    value = _field(receipt, name)
    return value if isinstance(value, str) and value else None


def _string_set_field(receipt: Node, name: str) -> frozenset[str]:
    value = _field(receipt, name)
    if isinstance(value, list):
        return frozenset(item for item in value if isinstance(item, str) and item)
    if isinstance(value, str) and value:
        return frozenset(part.strip() for part in value.split(",") if part.strip())
    return frozenset()


def _field(receipt: Node, name: str) -> object:
    nested = receipt.fields.get("fields")
    if isinstance(nested, Mapping) and name in nested:
        return nested[name]
    return receipt.fields.get(name)
