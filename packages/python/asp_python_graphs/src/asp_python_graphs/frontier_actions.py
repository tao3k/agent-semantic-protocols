"""Typed graph-turbo frontier action projection."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field

from .model import FrontierEntry, GraphResult, Node
from .selector import graph_turbo_owner_path_for_node, graph_turbo_selector_for_node


@dataclass(frozen=True)
class FrontierAction:
    rank: int
    action_id: str
    action_kind: str
    selector: str | None
    owner: str
    symbol: str | None
    source_node_id: str
    next: str
    capability_id: str
    target: str
    target_role: str
    fields: Mapping[str, object] = field(default_factory=dict)


def frontier_action_items(result: GraphResult) -> list[FrontierAction]:
    if result.profile.name == "owner-query":
        return _owner_query_frontier_action_items(result)
    if result.profile.name == "failure-frontier":
        return _failure_frontier_action_items(result)
    return []


def frontier_action_packets(result: GraphResult) -> list[dict[str, object]]:
    return [_frontier_action_to_packet(action) for action in frontier_action_items(result)]


def owner_query_frontier_action_items(result: GraphResult) -> list[dict[str, object]]:
    return [
        _frontier_action_to_packet(action)
        for action in _owner_query_frontier_action_items(result)
    ]


def _owner_query_frontier_action_items(result: GraphResult) -> list[FrontierAction]:
    actions: list[FrontierAction] = []
    branches = _owner_query_branches(result)
    for index, node in enumerate(branches, start=1):
        selector = _selector_for_node(node)
        if selector is None:
            continue
        owner = _owner_for_node(node)
        if owner is None:
            continue
        symbol = _symbol_for_node(node)
        actions.append(
            FrontierAction(
                rank=index,
                action_id=f"S{index}",
                action_kind="selector",
                selector=selector,
                owner=owner,
                symbol=symbol,
                source_node_id=node.id,
                next="query-selector",
                capability_id="query",
                target=selector,
                target_role="selector",
                fields={
                    "ownerPath": owner,
                    "symbol": "" if symbol is None else symbol,
                    "sourceNodeId": node.id,
                },
            )
        )
        actions.append(
            FrontierAction(
                rank=index,
                action_id=f"R{index}",
                action_kind="reasoning",
                selector=None,
                owner=owner,
                symbol=None,
                source_node_id=node.id,
                next="search-reasoning",
                capability_id="search-reasoning",
                target=owner,
                target_role="owner",
                fields={
                    "ownerPath": owner,
                    "sourceNodeId": node.id,
                },
            )
        )
    return actions


def _failure_frontier_action_items(result: GraphResult) -> list[FrontierAction]:
    actions: list[FrontierAction] = []
    for entry in _prompt_frontier_entries(result):
        node = entry.node
        if node.kind != "hot" or entry.action != "code":
            continue
        selector = _selector_for_node(node)
        if selector is None:
            continue
        owner = _owner_for_node(node) or selector
        language = str(node.fields.get("languageId") or "rust")
        symbol = _symbol_for_node(node)
        actions.append(
            FrontierAction(
                rank=len(actions) + 1,
                action_id=f"C{len(actions) + 1}",
                action_kind="query-code",
                selector=selector,
                owner=owner,
                symbol=symbol,
                source_node_id=node.id,
                next="query-code",
                capability_id="query",
                target=selector,
                target_role="selector",
                fields={
                    "languageId": language,
                    "selector": selector,
                    "ownerPath": owner,
                    "symbol": "" if symbol is None else symbol,
                    "sourceNodeId": node.id,
                },
            )
        )
    return actions


def _frontier_action_to_packet(action: FrontierAction) -> dict[str, object]:
    return {
        "rank": action.rank,
        "actionId": action.action_id,
        "actionKind": action.action_kind,
        "selector": action.selector,
        "owner": action.owner,
        "symbol": action.symbol,
        "sourceNodeId": action.source_node_id,
        "next": action.next,
        "capabilityId": action.capability_id,
        "target": action.target,
        "targetRole": action.target_role,
        "fields": dict(action.fields),
    }


def _owner_query_branches(result: GraphResult) -> list[Node]:
    hot_by_key = _owner_query_hot_nodes(result)
    candidates = [
        entry.node
        for entry in _prompt_frontier_entries(result)
        if entry.node.kind in {"field", "hot", "item"}
        and _selector_for_node(entry.node)
    ]
    field_candidates = [node for node in candidates if node.kind == "field"]
    if field_candidates:
        return _dedupe_mapped_branches(
            _owner_query_diverse_candidates(field_candidates),
            hot_by_key,
        )
    return _dedupe_mapped_branches(
        _owner_query_diverse_candidates(candidates),
        hot_by_key,
    )


def _dedupe_mapped_branches(
    branches: list[Node], hot_by_key: Mapping[tuple[str, str], Node]
) -> list[Node]:
    selected: list[Node] = []
    seen_selectors: set[str] = set()
    for node in branches:
        mapped = _hot_node_for_branch(node, hot_by_key)
        selector = _selector_for_node(mapped)
        if selector is None or selector in seen_selectors:
            continue
        seen_selectors.add(selector)
        selected.append(mapped)
    return selected


def _owner_query_diverse_candidates(candidates: list[Node]) -> list[Node]:
    preferred = [node for node in candidates if _source_preferred_node(node)]
    diverse: list[Node] = []
    fallback: list[Node] = []
    seen_selectors: set[str] = set()
    seen_symbols: set[str] = set()
    _append_symbol_diverse_branches(
        preferred, diverse, fallback, seen_selectors, seen_symbols
    )
    if len(diverse) < 3:
        _append_symbol_diverse_branches(
            candidates, diverse, fallback, seen_selectors, seen_symbols
        )
    for node in fallback:
        if len(diverse) >= 3:
            break
        selector = _selector_for_node(node)
        if selector is None or selector in seen_selectors:
            continue
        seen_selectors.add(selector)
        diverse.append(node)
    return diverse


def _owner_query_hot_nodes(result: GraphResult) -> dict[tuple[str, str], Node]:
    hot_nodes: dict[tuple[str, str], Node] = {}
    for entry in _prompt_frontier_entries(result):
        node = entry.node
        if node.kind != "hot" or _selector_for_node(node) is None:
            continue
        hot_nodes.setdefault(_branch_key(node), node)
    return hot_nodes


def _hot_node_for_branch(
    node: Node, hot_by_key: Mapping[tuple[str, str], Node]
) -> Node:
    if node.kind == "hot":
        return node
    return hot_by_key.get(_branch_key(node), node)


def _branch_key(node: Node) -> tuple[str, str]:
    owner = str(node.fields.get("ownerPath") or node.fields.get("path") or "")
    return owner, _branch_symbol(node)


def _append_symbol_diverse_branches(
    nodes: list[Node],
    diverse: list[Node],
    fallback: list[Node],
    seen_selectors: set[str],
    seen_symbols: set[str],
) -> None:
    for node in nodes:
        if len(diverse) >= 3:
            return
        selector = _selector_for_node(node)
        if selector is None or selector in seen_selectors:
            continue
        symbol = _branch_symbol(node)
        if symbol in seen_symbols:
            fallback.append(node)
            continue
        seen_selectors.add(selector)
        seen_symbols.add(symbol)
        diverse.append(node)


def _branch_symbol(node: Node) -> str:
    fields = node.fields.get("fields")
    field_name = fields.get("fieldName") if isinstance(fields, Mapping) else None
    return str(node.fields.get("symbol") or field_name or node.value).lower()


def _source_preferred_node(node: Node) -> bool:
    path = str(node.fields.get("path") or node.fields.get("ownerPath") or "")
    return not (
        "/tests/" in path
        or path.endswith("/tests")
        or "/benches/" in path
        or path.endswith("/benches")
        or "/examples/" in path
        or path.endswith("/examples")
        or "stress-test/" in path
    )


def _prompt_frontier_entries(result: GraphResult) -> tuple[FrontierEntry, ...]:
    if result.profile.name != "failure-frontier":
        return result.frontier
    rank_index = {node.id: index for index, node in enumerate(result.ranked_nodes)}
    entries = [
        entry
        for entry in result.frontier
        if entry.node.kind in {"assert", "hot", "key", "evidence"}
    ]
    entries.sort(
        key=lambda entry: (
            {"assert": 0, "hot": 1, "key": 2, "evidence": 3}.get(entry.node.kind, 9),
            rank_index.get(entry.node.id, 999),
        )
    )
    return tuple(entries) if entries else result.frontier


def _selector_for_node(node: Node) -> str | None:
    return graph_turbo_selector_for_node(node)


def _owner_for_node(node: Node) -> str | None:
    owner = graph_turbo_owner_path_for_node(node)
    return None if owner is None else str(owner)


def _symbol_for_node(node: Node) -> str | None:
    fields = node.fields.get("fields")
    field_name = fields.get("fieldName") if isinstance(fields, Mapping) else None
    symbol = node.fields.get("symbol") or field_name or node.value
    return None if symbol is None else str(symbol)
