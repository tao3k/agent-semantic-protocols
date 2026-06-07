"""Receipt graph facts used as deterministic ranking feedback."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from itertools import groupby
from operator import attrgetter

from .diversity import selector_for_node
from .model import Edge, Node, ReceiptAdjustment, TypedGraph

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
        selector = selector_for_node(node)
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
    weight = _numeric(edge.fields.get("scoreDelta"))
    if weight is not None:
        return weight
    if effect == "boost":
        return _DEFAULT_BOOST
    return _DEFAULT_PENALTY


def _receipt_selector(receipt: Node) -> str | None:
    receipt_kind = receipt.fields.get("receiptKind") or receipt.role
    if str(receipt_kind) not in _READ_RECEIPT_KINDS:
        return None
    selector = receipt.fields.get("selector") or receipt.fields.get("locator")
    if isinstance(selector, str) and selector:
        return selector
    return None


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
        node_selector = selector_for_node(node)
        node_range = _node_range(node, node_selector)
        if node_range is None or node_selector == receipt_selector:
            continue
        if not _ranges_overlap(receipt_range, node_range):
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
    reasons = receipt.fields.get("avoidReasons") or receipt.fields.get("avoid")
    if isinstance(reasons, list):
        return frozenset(str(reason) for reason in reasons if reason)
    if isinstance(reasons, str) and reasons:
        return frozenset(part.strip() for part in reasons.split(",") if part.strip())
    return frozenset()


def _overlap_penalty_enabled(reasons: frozenset[str]) -> bool:
    return bool(reasons & {"manual-window-scan", "overlapping-range", "same-range-overlap"})


def _same_owner_penalty_enabled(reasons: frozenset[str]) -> bool:
    return bool(reasons & {"repeat-owner", "same-owner-scan"})


def _raw_read_penalty_enabled(receipt: Node, reasons: frozenset[str]) -> bool:
    receipt_kind = str(receipt.fields.get("receiptKind") or receipt.role)
    return receipt_kind == "raw-read" or "raw-read" in reasons or "raw-read-fallback" in reasons


def _receipt_range(receipt: Node) -> tuple[str, int, int] | None:
    selector = _receipt_selector(receipt)
    if selector is not None and (parsed := _parse_selector(selector)) is not None:
        return parsed
    return _range_from_fields(receipt.fields)


def _node_range(node: Node, selector: str | None) -> tuple[str, int, int] | None:
    if selector is not None and (parsed := _parse_selector(selector)) is not None:
        return parsed
    return _range_from_fields(node.fields)


def _receipt_owner_path(receipt: Node) -> str | None:
    owner = receipt.fields.get("ownerPath") or receipt.fields.get("path")
    if isinstance(owner, str) and owner:
        return owner
    if (receipt_range := _receipt_range(receipt)) is not None:
        return receipt_range[0]
    return None


def _node_owner_path(node: Node) -> str | None:
    owner = node.fields.get("ownerPath") or node.fields.get("owner")
    if isinstance(owner, str) and owner:
        return owner
    path = node.fields.get("path")
    if isinstance(path, str) and path:
        return path
    return None


def _range_from_fields(fields: Mapping[str, object]) -> tuple[str, int, int] | None:
    path = fields.get("path")
    start = fields.get("startLine") or fields.get("start")
    end = fields.get("endLine") or fields.get("end")
    if isinstance(path, str) and isinstance(start, int) and isinstance(end, int):
        return path, start, end
    return None


def _parse_selector(selector: str) -> tuple[str, int, int] | None:
    path, start, end = _parse_colon_range(selector)
    if path is not None:
        return path, start, end
    path, start, end = _parse_dash_range(selector)
    if path is not None:
        return path, start, end
    return None


def _parse_colon_range(selector: str) -> tuple[str | None, int, int]:
    path, sep, end_text = selector.rpartition(":")
    if not sep:
        return None, 0, 0
    path, sep, start_text = path.rpartition(":")
    if not sep:
        return None, 0, 0
    try:
        start = int(start_text)
        end = int(end_text)
    except ValueError:
        return None, 0, 0
    if not path or end < start:
        return None, 0, 0
    return path, start, end


def _parse_dash_range(selector: str) -> tuple[str | None, int, int]:
    path, sep, range_text = selector.rpartition(":")
    if not sep:
        return None, 0, 0
    start_text, sep, end_text = range_text.partition("-")
    if not sep:
        return None, 0, 0
    try:
        start = int(start_text)
        end = int(end_text)
    except ValueError:
        return None, 0, 0
    if not path or end < start:
        return None, 0, 0
    return path, start, end


def _ranges_overlap(
    left: tuple[str, int, int], right: tuple[str, int, int]
) -> bool:
    left_path, left_start, left_end = left
    right_path, right_start, right_end = right
    return left_path == right_path and left_start <= right_end and right_start <= left_end


def _numeric(value: object) -> float | None:
    if isinstance(value, int | float):
        return float(value)
    return None
