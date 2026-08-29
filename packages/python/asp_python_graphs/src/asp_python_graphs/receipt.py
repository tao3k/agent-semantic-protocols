"""Receipt graph facts used as deterministic ranking feedback."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from itertools import groupby
from operator import attrgetter

from .model import Edge, Node, ReceiptAdjustment, TypedGraph
from .selector import (
    GraphTurboSelectorRange,
    graph_turbo_node_range,
    graph_turbo_owner_path_for_node,
    graph_turbo_parse_selector,
    graph_turbo_range_from_fields,
    graph_turbo_ranges_overlap,
    graph_turbo_selector_for_node,
)

_BOOST_RELATIONS = {
    "exact-code-success",
    "failure-evidence-used",
    "frontier-followed",
    "test-passed",
    "used-evidence",
    "validated",
    "validates",
}
_PENALTY_RELATIONS = {
    "duplicate-read",
    "ignored",
    "manual-window-scan",
    "overlaps",
    "raw-read-fallback",
    "read",
    "same-owner-scan",
    "same-range-overlap",
}
_READ_RECEIPT_KINDS = {"direct-read", "raw-read", "read", "query-code"}
_SUCCESS_RECEIPT_KINDS = {"answer-evidence", "frontier-success", "validated-selector"}
_WASTE_RECEIPT_KINDS = {"extra-selector", "frontier-waste", "wasted-selector"}
_DEFAULT_BOOST = 0.45
_DEFAULT_PENALTY = -0.75


def receipt_score_adjustments(
    graph: TypedGraph,
) -> tuple[dict[str, float], tuple[ReceiptAdjustment, ...]]:
    adjustments: list[ReceiptAdjustment] = []
    receipt_nodes = {
        node.id: node for node in graph.nodes.values() if node.kind == "receipt"
    }
    if not receipt_nodes:
        return {}, ()

    _extend_edge_adjustments(graph, receipt_nodes, adjustments)
    _extend_feedback_selector_adjustments(graph, receipt_nodes.values(), adjustments)
    _extend_selector_adjustments(graph, receipt_nodes.values(), adjustments)
    _extend_read_pattern_adjustments(graph, receipt_nodes.values(), adjustments)
    return _score_by_node(adjustments), tuple(adjustments)


def receipt_adjustment_counts(
    adjustments: tuple[ReceiptAdjustment, ...],
) -> tuple[int, int]:
    boost_count = sum(1 for adjustment in adjustments if adjustment.score_delta > 0.0)
    penalty_count = sum(1 for adjustment in adjustments if adjustment.score_delta < 0.0)
    return boost_count, penalty_count


def receipt_reasons_by_node(
    adjustments: tuple[ReceiptAdjustment, ...],
) -> dict[str, tuple[str, ...]]:
    reasons: dict[str, list[str]] = {}
    for adjustment in adjustments:
        reasons.setdefault(adjustment.node_id, []).append(
            f"receipt-{adjustment.effect}:{adjustment.score_delta:+.2f}:{adjustment.reason}"
        )
    return {node_id: tuple(values) for node_id, values in reasons.items()}


def _score_by_node(adjustments: Iterable[ReceiptAdjustment]) -> dict[str, float]:
    key = attrgetter("node_id")
    return {
        node_id: sum(adjustment.score_delta for adjustment in grouped_adjustments)
        for node_id, grouped_adjustments in groupby(sorted(adjustments, key=key), key)
    }


def _extend_edge_adjustments(
    graph: TypedGraph,
    receipt_nodes: Mapping[str, Node],
    adjustments: list[ReceiptAdjustment],
) -> None:
    for edge in graph.edges:
        _receipt_id, target_id = _receipt_edge_target(edge, receipt_nodes)
        if target_id is None:
            continue
        effect = _relation_effect(edge.relation)
        if effect is None:
            continue
        adjustments.append(
            ReceiptAdjustment(
                node_id=target_id,
                effect=effect,
                score_delta=_edge_score_delta(edge, effect),
                reason=edge.relation,
            )
        )


def _extend_selector_adjustments(
    graph: TypedGraph,
    receipts: Iterable[Node],
    adjustments: list[ReceiptAdjustment],
) -> None:
    seen_selectors = {
        selector
        for selector in (_receipt_selector(receipt) for receipt in receipts)
        if selector
    }
    if not seen_selectors:
        return
    for node in graph.nodes.values():
        if node.kind == "receipt":
            continue
        selector = graph_turbo_selector_for_node(node)
        if selector not in seen_selectors:
            continue
        adjustments.append(
            ReceiptAdjustment(
                node_id=node.id,
                effect="penalty",
                score_delta=_DEFAULT_PENALTY,
                reason="seen-selector",
            )
        )


def _extend_feedback_selector_adjustments(
    graph: TypedGraph,
    receipts: Iterable[Node],
    adjustments: list[ReceiptAdjustment],
) -> None:
    for receipt in receipts:
        _extend_single_feedback_selector_adjustment(graph, receipt, adjustments)


def _extend_single_feedback_selector_adjustment(
    graph: TypedGraph,
    receipt: Node,
    adjustments: list[ReceiptAdjustment],
) -> None:
    selector = _receipt_selector_value(receipt)
    effect = _selector_feedback_effect(receipt)
    if selector is None or effect is None:
        return
    score_delta = _receipt_score_delta(receipt, effect)
    reason = _receipt_reason(receipt, effect)
    matched_nodes = _selector_target_nodes(graph, selector, receipt)
    for node in matched_nodes:
        adjustments.append(
            ReceiptAdjustment(
                node_id=node.id,
                effect=effect,
                score_delta=score_delta,
                reason=reason,
            )
        )
    _extend_feedback_relation_propagation(
        graph,
        receipt,
        matched_nodes,
        effect=effect,
        score_delta=score_delta,
        reason=reason,
        adjustments=adjustments,
    )


def _selector_target_nodes(
    graph: TypedGraph,
    selector: str,
    receipt: Node,
) -> tuple[Node, ...]:
    target_kinds = _string_set_field(receipt.fields, "targetKinds")
    return tuple(
        node
        for node in graph.nodes.values()
        if node.kind != "receipt"
        and graph_turbo_selector_for_node(node) == selector
        and (not target_kinds or node.kind in target_kinds)
    )


def _extend_feedback_relation_propagation(
    graph: TypedGraph,
    receipt: Node,
    matched_nodes: tuple[Node, ...],
    *,
    effect: str,
    score_delta: float,
    reason: str,
    adjustments: list[ReceiptAdjustment],
) -> None:
    if _feedback_scope(receipt) != "relation-neighborhood":
        return
    relations = _string_set_field(receipt.fields, "propagateRelations")
    if not relations:
        return
    propagate_kinds = _string_set_field(receipt.fields, "propagateKinds")
    factor = _numeric(_field(receipt.fields, "propagationFactor"))
    propagated_delta = score_delta * (0.35 if factor is None else factor)
    for node in matched_nodes:
        for neighbor, relation in _feedback_neighbors(graph, node.id, relations):
            if propagate_kinds and neighbor.kind not in propagate_kinds:
                continue
            adjustments.append(
                ReceiptAdjustment(
                    node_id=neighbor.id,
                    effect=effect,
                    score_delta=propagated_delta,
                    reason=f"{reason}:{relation}",
                )
            )


def _feedback_neighbors(
    graph: TypedGraph,
    node_id: str,
    relations: frozenset[str],
) -> tuple[tuple[Node, str], ...]:
    neighbors: list[tuple[Node, str]] = []
    for edge in graph.edges:
        if edge.relation not in relations:
            continue
        if edge.source == node_id and edge.target in graph.nodes:
            neighbors.append((graph.nodes[edge.target], edge.relation))
        elif edge.target == node_id and edge.source in graph.nodes:
            neighbors.append((graph.nodes[edge.source], edge.relation))
    return tuple(neighbors)


def _feedback_scope(receipt: Node) -> str:
    scope = _field(receipt.fields, "scope")
    if isinstance(scope, str) and scope:
        return scope
    return "exact-selector"


def _extend_read_pattern_adjustments(
    graph: TypedGraph,
    receipts: Iterable[Node],
    adjustments: list[ReceiptAdjustment],
) -> None:
    for receipt in receipts:
        reasons = _receipt_avoid_reasons(receipt)
        if _overlap_penalty_enabled(reasons):
            _extend_overlap_adjustments(graph, receipt, adjustments)
        if _same_owner_penalty_enabled(reasons):
            _extend_same_owner_adjustments(
                graph, receipt, adjustments, reason="same-owner-scan"
            )
        if _raw_read_penalty_enabled(receipt, reasons):
            _extend_same_owner_adjustments(
                graph, receipt, adjustments, reason="raw-read-fallback"
            )


def _receipt_edge_target(
    edge: Edge, receipt_nodes: Mapping[str, Node]
) -> tuple[str | None, str | None]:
    if edge.source in receipt_nodes and edge.target not in receipt_nodes:
        return edge.source, edge.target
    if edge.target in receipt_nodes and edge.source not in receipt_nodes:
        return edge.target, edge.source
    return None, None


def _relation_effect(relation: str) -> str | None:
    if relation in _BOOST_RELATIONS:
        return "boost"
    if relation in _PENALTY_RELATIONS:
        return "penalty"
    return None


def _edge_score_delta(edge: Edge, effect: str) -> float:
    weight = _numeric(_field(edge.fields, "scoreDelta"))
    if weight is not None:
        return weight
    if effect == "boost":
        return _DEFAULT_BOOST
    return _DEFAULT_PENALTY


def _receipt_selector(receipt: Node) -> str | None:
    receipt_kind = _field(receipt.fields, "receiptKind") or receipt.role
    if str(receipt_kind) not in _READ_RECEIPT_KINDS:
        return None
    return _receipt_selector_value(receipt)


def _receipt_selector_value(receipt: Node) -> str | None:
    selector = _field(receipt.fields, "selector") or _field(receipt.fields, "locator")
    if isinstance(selector, str) and selector:
        return selector
    return None


def _selector_feedback_effect(receipt: Node) -> str | None:
    explicit_effect = _field(receipt.fields, "effect")
    if explicit_effect in {"boost", "penalty"}:
        return str(explicit_effect)
    receipt_kind = str(_field(receipt.fields, "receiptKind") or receipt.role)
    if receipt_kind in _SUCCESS_RECEIPT_KINDS:
        return "boost"
    if receipt_kind in _WASTE_RECEIPT_KINDS:
        return "penalty"
    return None


def _receipt_score_delta(receipt: Node, effect: str) -> float:
    score_delta = _numeric(_field(receipt.fields, "scoreDelta"))
    if score_delta is not None:
        return score_delta
    if effect == "boost":
        return _DEFAULT_BOOST
    return _DEFAULT_PENALTY


def _receipt_reason(receipt: Node, effect: str) -> str:
    reason = _field(receipt.fields, "reason")
    if isinstance(reason, str) and reason:
        return reason
    return "frontier-success" if effect == "boost" else "frontier-waste"


def _extend_overlap_adjustments(
    graph: TypedGraph, receipt: Node, adjustments: list[ReceiptAdjustment]
) -> None:
    receipt_range = _receipt_range(receipt)
    if receipt_range is None:
        return
    receipt_selector = _receipt_selector(receipt)
    for node in graph.nodes.values():
        if node.kind == "receipt":
            continue
        node_selector = graph_turbo_selector_for_node(node)
        node_range = _node_range(node, node_selector)
        if node_range is None or node_selector == receipt_selector:
            continue
        if not graph_turbo_ranges_overlap(receipt_range, node_range):
            continue
        adjustments.append(
            ReceiptAdjustment(
                node_id=node.id,
                effect="penalty",
                score_delta=_DEFAULT_PENALTY,
                reason="same-range-overlap",
            )
        )


def _extend_same_owner_adjustments(
    graph: TypedGraph,
    receipt: Node,
    adjustments: list[ReceiptAdjustment],
    *,
    reason: str,
) -> None:
    owner = _receipt_owner_path(receipt)
    if owner is None:
        return
    for node in graph.nodes.values():
        if node.kind == "receipt" or node.kind in {"query", "test"}:
            continue
        if _node_owner_path(node) != owner:
            continue
        adjustments.append(
            ReceiptAdjustment(
                node_id=node.id,
                effect="penalty",
                score_delta=_DEFAULT_PENALTY,
                reason=reason,
            )
        )


def _receipt_avoid_reasons(receipt: Node) -> frozenset[str]:
    reasons = _field(receipt.fields, "avoidReasons") or _field(receipt.fields, "avoid")
    if isinstance(reasons, list):
        return frozenset(str(reason) for reason in reasons if reason)
    if isinstance(reasons, str) and reasons:
        return frozenset(part.strip() for part in reasons.split(",") if part.strip())
    return frozenset()


def _overlap_penalty_enabled(reasons: frozenset[str]) -> bool:
    return bool(
        reasons & {"manual-window-scan", "overlapping-range", "same-range-overlap"}
    )


def _same_owner_penalty_enabled(reasons: frozenset[str]) -> bool:
    return bool(reasons & {"repeat-owner", "same-owner-scan"})


def _raw_read_penalty_enabled(receipt: Node, reasons: frozenset[str]) -> bool:
    receipt_kind = str(_field(receipt.fields, "receiptKind") or receipt.role)
    return (
        receipt_kind == "raw-read"
        or "raw-read" in reasons
        or "raw-read-fallback" in reasons
    )


def _receipt_range(receipt: Node) -> GraphTurboSelectorRange | None:
    selector = _receipt_selector(receipt)
    if selector is not None and (parsed := graph_turbo_parse_selector(selector)):
        return parsed
    return graph_turbo_range_from_fields(receipt.fields)


def _node_range(node: Node, selector: str | None) -> GraphTurboSelectorRange | None:
    if selector is not None and (parsed := graph_turbo_parse_selector(selector)):
        return parsed
    return graph_turbo_node_range(node)


def _receipt_owner_path(receipt: Node) -> str | None:
    owner = _field(receipt.fields, "ownerPath") or _field(receipt.fields, "path")
    if isinstance(owner, str) and owner:
        return owner
    if (receipt_range := _receipt_range(receipt)) is not None:
        return receipt_range.path
    return None


def _node_owner_path(node: Node) -> str | None:
    return graph_turbo_owner_path_for_node(node)


def _numeric(value: object) -> float | None:
    if isinstance(value, int | float):
        return float(value)
    return None


def _string_set_field(fields: Mapping[str, object], name: str) -> frozenset[str]:
    value = _field(fields, name)
    if isinstance(value, list):
        return frozenset(item for item in value if isinstance(item, str) and item)
    if isinstance(value, str) and value:
        return frozenset(part.strip() for part in value.split(",") if part.strip())
    return frozenset()


def _field(fields: Mapping[str, object], name: str) -> object:
    nested = fields.get("fields")
    if isinstance(nested, Mapping) and name in nested:
        return nested[name]
    return fields.get(name)
