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
from .query_token_balance import (
    query_token_balance_bonus,
    query_tokens_for_node,
    repair_query_token_coverage,
)
from .semantic_fact_coverage import repair_semantic_fact_coverage
from .selector import (
    graph_turbo_node_range,
    graph_turbo_owner_path_for_node,
    graph_turbo_ranges_adjacent,
    graph_turbo_selector_for_node,
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
    query_tokens: tuple[str, ...] = (),
    coverage_limit: int | None = None,
) -> tuple[Node, ...]:
    remaining = [
        graph.nodes[node_id]
        for node_id in scores
        if graph_turbo_selector_for_node(graph.nodes[node_id]) not in seen_selectors
    ]
    ranked: list[Node] = []
    selected_kind_counts: dict[str, int] = {}
    covered_query_tokens: set[str] = set()
    while remaining and len(ranked) < limit:
        remaining.sort(
            key=lambda node: (
                -_adjusted_score(
                    node,
                    ranked,
                    scores[node.id],
                    selected_kind_counts,
                    query_tokens,
                    covered_query_tokens,
                ),
                best_depth.get(node.id, 99),
                node.kind,
                node.id,
            )
        )
        node = _pop_next_ranked_node(remaining, selected_kind_counts, kind_budgets)
        if node is None:
            break
        ranked.append(node)
        covered_query_tokens.update(
            query_tokens_for_node(node, query_tokens, include_query_node=False)
        )
        selected_kind_counts[node.kind] = selected_kind_counts.get(node.kind, 0) + 1
    repaired = repair_query_token_coverage(
        ranked,
        remaining,
        scores,
        query_tokens,
        coverage_limit or limit,
    )
    repaired = repair_semantic_fact_coverage(
        graph,
        repaired,
        scores,
        query_tokens,
        coverage_limit or limit,
    )
    return tuple(repaired)


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
    query_tokens: tuple[str, ...] = (),
    covered_query_tokens: set[str] | None = None,
) -> float:
    adjusted = score
    if _has_same_owner_penalty(node, selected):
        adjusted -= SAME_OWNER_PENALTY
    if _has_same_symbol_penalty(node, selected):
        adjusted -= SAME_SYMBOL_NAME_PENALTY
    if selected_kind_counts.get(node.kind, 0) > 0:
        adjusted -= SAME_KIND_OVER_BUDGET_PENALTY
    if _has_contiguous_window(node, selected):
        adjusted += CONTIGUOUS_WINDOW_MERGE_BONUS
    adjusted += query_token_balance_bonus(
        node,
        query_tokens,
        covered_query_tokens or set(),
    )
    return adjusted


def _has_same_owner_penalty(
    node: Node, selected: tuple[Node, ...] | list[Node]
) -> bool:
    owner = _owner_key(node)
    if owner is None:
        return False
    return any(
        _owner_key(item) == owner and not _hot_companion_pair(node, item)
        for item in selected
    )


def _has_same_symbol_penalty(
    node: Node, selected: tuple[Node, ...] | list[Node]
) -> bool:
    symbol = _symbol_name(node)
    if symbol is None:
        return False
    return any(
        _symbol_name(item) == symbol and not _hot_companion_pair(node, item)
        for item in selected
    )


def _hot_companion_pair(left: Node, right: Node) -> bool:
    if {left.kind, right.kind} - {"field", "hot", "item", "symbol"}:
        return False
    if left.kind == right.kind or "hot" not in {left.kind, right.kind}:
        return False
    if _owner_key(left) is None or _owner_key(left) != _owner_key(right):
        return False
    if _symbol_name(left) is None or _symbol_name(left) != _symbol_name(right):
        return False
    left_range = graph_turbo_node_range(left)
    right_range = graph_turbo_node_range(right)
    if left_range is None or right_range is None:
        return False
    if left_range.path != right_range.path:
        return False
    hot_range, locator_range = (
        (left_range, right_range) if left.kind == "hot" else (right_range, left_range)
    )
    return (
        hot_range.start_line <= locator_range.start_line <= hot_range.end_line
        and hot_range.start_line <= locator_range.end_line <= hot_range.end_line
    )


def _owner_key(node: Node) -> str | None:
    return graph_turbo_owner_path_for_node(node)


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
    for item in selected:
        other = _window_bounds(item)
        if other is None:
            continue
        if graph_turbo_ranges_adjacent(window, other, max_gap_lines=1):
            return True
    return False


def _window_bounds(node: Node):
    if node.kind not in {"range", "window"}:
        return None
    return graph_turbo_node_range(node)


def selector_for_node(node: Node) -> str | None:
    return graph_turbo_selector_for_node(node)
