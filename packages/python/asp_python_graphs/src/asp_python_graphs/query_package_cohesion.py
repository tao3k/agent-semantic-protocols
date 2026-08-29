"""Path-cluster package cohesion scoring for graph-turbo ranking."""

from __future__ import annotations

from collections import Counter
from collections.abc import Iterable, Mapping

from .model import Node, TypedGraph
from .query_tokens import GENERIC_PATH_TOKENS, query_tokens_from_text

_PACKAGE_COHESION_BONUS = 0.45
_PACKAGE_DRIFT_PENALTY = 0.35
_PATH_TOKEN_COMMON_MIN_COUNT = 3
_PATH_TOKEN_COMMON_RATIO = 0.35


def query_package_cohesion_adjustment(
    graph: TypedGraph,
    *,
    profile_name: str,
    seed_ids: Iterable[str],
    node: Node,
    package_tokens: Iterable[str] | None = None,
) -> float:
    if profile_name != "owner-query" or node.kind == "query":
        return 0.0
    package_token_tuple = (
        tuple(package_tokens)
        if package_tokens is not None
        else query_package_cohesion_tokens(graph, seed_ids)
    )
    if not package_token_tuple:
        return 0.0
    path_text = _node_path_text(node)
    matched_path_tokens = tuple(
        token for token in package_token_tuple if token in path_text
    )
    if matched_path_tokens:
        if len(package_token_tuple) <= 2:
            return _PACKAGE_COHESION_BONUS
        return (
            _PACKAGE_COHESION_BONUS
            * len(matched_path_tokens)
            / len(package_token_tuple)
        )
    node_text = _node_text(node)
    if any(token in node_text for token in package_token_tuple):
        return -_PACKAGE_DRIFT_PENALTY
    return 0.0


def query_package_cohesion_tokens(
    graph: TypedGraph,
    seed_ids: Iterable[str],
) -> tuple[str, ...]:
    return _query_package_tokens(graph, seed_ids)


def _query_package_tokens(
    graph: TypedGraph,
    seed_ids: Iterable[str],
) -> tuple[str, ...]:
    path_token_counts, path_node_count = _path_token_counts(graph)
    path_tokens = set(path_token_counts)
    candidate_tokens = tuple(
        token
        for token in _query_tokens(graph, seed_ids)
        if "_" in token or (token in path_tokens and token not in GENERIC_PATH_TOKENS)
    )
    specific_tokens = tuple(
        token
        for token in candidate_tokens
        if "_" in token
        or _is_specific_path_token(
            token,
            path_token_counts=path_token_counts,
            path_node_count=path_node_count,
        )
    )
    return specific_tokens or candidate_tokens


def _query_tokens(graph: TypedGraph, seed_ids: Iterable[str]) -> tuple[str, ...]:
    tokens: list[str] = []
    seen: set[str] = set()
    for seed_id in seed_ids:
        node = graph.nodes.get(seed_id)
        if node is None or node.kind != "query":
            continue
        for token in _tokens(str(node.value)):
            if token in seen:
                continue
            seen.add(token)
            tokens.append(token)
    return tuple(tokens)


def _node_text(node: Node) -> str:
    parts = [
        _node_semantic_text(node),
        str(node.fields.get("path") or ""),
        str(node.fields.get("ownerPath") or ""),
    ]
    return " ".join(parts).lower()


def _node_semantic_text(node: Node) -> str:
    parts = [
        node.value,
        node.role,
        node.kind,
        str(node.fields.get("symbol") or ""),
        str(node.fields.get("name") or ""),
        str(node.fields.get("matchText") or ""),
        _node_fields_text(node),
        _semantic_alias_text(node),
    ]
    return " ".join(parts).lower()


def _node_path_text(node: Node) -> str:
    parts = [
        str(node.fields.get("path") or ""),
        str(node.fields.get("ownerPath") or ""),
    ]
    if node.kind in {"owner", "test"}:
        parts.append(node.value)
    return " ".join(parts).lower()


def _node_fields_text(node: Node) -> str:
    fields = node.fields.get("fields")
    if not isinstance(fields, Mapping):
        return ""
    return " ".join(str(value) for value in fields.values())


def _semantic_alias_text(node: Node) -> str:
    aliases = {
        "collection": "collection collections list lists map maps set sets",
        "field": "field fields",
        "type": "type types",
    }
    return aliases.get(node.kind, "")


def _tokens(value: str) -> tuple[str, ...]:
    return query_tokens_from_text(value)


def _path_token_counts(graph: TypedGraph) -> tuple[Counter[str], int]:
    token_counts: Counter[str] = Counter()
    node_count = 0
    for node in graph.nodes.values():
        node_tokens = set(_tokens(_node_path_text(node)))
        if not node_tokens:
            continue
        node_count += 1
        token_counts.update(node_tokens)
    return token_counts, node_count


def _is_specific_path_token(
    token: str,
    *,
    path_token_counts: Mapping[str, int],
    path_node_count: int,
) -> bool:
    count = path_token_counts.get(token, 0)
    if count <= 0 or path_node_count <= 0:
        return False
    if count < _PATH_TOKEN_COMMON_MIN_COUNT:
        return True
    return count / path_node_count <= _PATH_TOKEN_COMMON_RATIO
