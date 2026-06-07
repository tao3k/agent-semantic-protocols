"""Diversity and per-kind ranking helpers."""

from __future__ import annotations

from collections.abc import Mapping

from .model import Node, TypedGraph
from .policy import (
    CONTIGUOUS_WINDOW_MERGE_BONUS,
    SAME_KIND_OVER_BUDGET_PENALTY,
    SAME_OWNER_PENALTY,
    SAME_SYMBOL_NAME_PENALTY,
)


def normalize_kind_budgets(kind_budgets: Mapping[str, int] | None) -> dict[str, int]:
    if kind_budgets is None:
        return {}
    return {
        kind: budget
        for kind, budget in kind_budgets.items()
        if isinstance(kind, str) and isinstance(budget, int) and budget > 0
    }


def rank_nodes(
    graph: TypedGraph,
    scores: Mapping[str, float],
    best_depth: Mapping[str, int],
    limit: int,
    kind_budgets: Mapping[str, int],
    seen_selectors: frozenset[str] = frozenset(),
) -> tuple[Node, ...]:
    remaining = [
        graph.nodes[node_id]
        for node_id in scores
        if _selector_for_node(graph.nodes[node_id]) not in seen_selectors
    ]
    ranked: list[Node] = []
    selected_kind_counts: dict[str, int] = {}
    while remaining and len(ranked) < limit:
        remaining.sort(
            key=lambda node: (
                -_adjusted_score(node, ranked, scores[node.id], selected_kind_counts),
                best_depth.get(node.id, 99),
                node.kind,
                node.id,
            )
        )
        node = _pop_next_ranked_node(remaining, selected_kind_counts, kind_budgets)
        if node is None:
            break
        ranked.append(node)
        selected_kind_counts[node.kind] = selected_kind_counts.get(node.kind, 0) + 1
    return tuple(ranked)


def _pop_next_ranked_node(
    remaining: list[Node],
    selected_kind_counts: Mapping[str, int],
    kind_budgets: Mapping[str, int],
) -> Node | None:
    for index, node in enumerate(remaining):
        budget = kind_budgets.get(node.kind)
        if budget is not None and selected_kind_counts.get(node.kind, 0) >= budget:
            continue
        return remaining.pop(index)
    return None


def _adjusted_score(
    node: Node,
    selected: tuple[Node, ...] | list[Node],
    score: float,
    selected_kind_counts: Mapping[str, int],
) -> float:
    adjusted = score
    if _owner_key(node) is not None and _owner_key(node) in {
        _owner_key(item) for item in selected
    }:
        adjusted -= SAME_OWNER_PENALTY
    if _symbol_name(node) is not None and _symbol_name(node) in {
        _symbol_name(item) for item in selected
    }:
        adjusted -= SAME_SYMBOL_NAME_PENALTY
    if selected_kind_counts.get(node.kind, 0) > 0:
        adjusted -= SAME_KIND_OVER_BUDGET_PENALTY
    if _has_contiguous_window(node, selected):
        adjusted += CONTIGUOUS_WINDOW_MERGE_BONUS
    return adjusted


def _owner_key(node: Node) -> str | None:
    value = node.fields.get("owner") or node.fields.get("ownerPath")
    if isinstance(value, str) and value:
        return value
    path = node.fields.get("path")
    if isinstance(path, str) and path:
        return path
    return None


def _symbol_name(node: Node) -> str | None:
    value = node.fields.get("symbol") or node.fields.get("name")
    if isinstance(value, str) and value:
        return value
    if node.kind in {"hot", "item", "symbol"}:
        return node.value
    return None


def _has_contiguous_window(node: Node, selected: tuple[Node, ...] | list[Node]) -> bool:
    window = _window_bounds(node)
    if window is None:
        return False
    path, start_line, end_line = window
    for item in selected:
        other = _window_bounds(item)
        if other is None:
            continue
        other_path, other_start, other_end = other
        if path == other_path and start_line <= other_end + 1 and other_start <= end_line + 1:
            return True
    return False


def _window_bounds(node: Node) -> tuple[str, int, int] | None:
    if node.kind not in {"range", "window"}:
        return None
    path = node.fields.get("path")
    start = node.fields.get("startLine") or node.fields.get("start")
    end = node.fields.get("endLine") or node.fields.get("end")
    if not isinstance(path, str) or not isinstance(start, int) or not isinstance(end, int):
        return None
    return path, start, end


def selector_for_node(node: Node) -> str | None:
    return _selector_for_node(node)


def _selector_for_node(node: Node) -> str | None:
    fields = node.fields.get("fields")
    if isinstance(fields, Mapping):
        context_locator = fields.get("contextLocator")
        if isinstance(context_locator, str) and context_locator:
            return context_locator
    locator = node.fields.get("locator") or node.fields.get("location")
    if isinstance(locator, str) and locator:
        return locator
    path = node.fields.get("path")
    start = node.fields.get("startLine") or node.fields.get("start")
    end = node.fields.get("endLine") or node.fields.get("end")
    if isinstance(path, str) and isinstance(start, int) and isinstance(end, int):
        return f"{path}:{start}:{end}"
    return None
