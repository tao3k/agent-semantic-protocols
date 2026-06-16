"""Query-token priority helpers for graph-turbo ranking."""

from __future__ import annotations

from collections.abc import Mapping

from .model import Node
from .query_tokens import query_tokens_from_text

QUERY_TOKEN_UNCOVERED_BONUS = 0.22
QUERY_TOKEN_PRIMARY_CLAUSE_MULTIPLIER = 3.0
QUERY_TOKEN_SECONDARY_CLAUSE_MULTIPLIER = 0.6


def prioritized_query_tokens(
    query_tokens: tuple[str, ...],
    query_clauses: tuple[str, ...] = (),
) -> tuple[str, ...]:
    clause_groups = _query_clause_token_groups(query_clauses)
    if len(clause_groups) < 2:
        return query_tokens
    selected: list[str] = []
    query_token_set = set(query_tokens)
    for clause in clause_groups:
        for token in clause:
            if token in query_token_set and token not in selected:
                selected.append(token)
    selected.extend(token for token in query_tokens if token not in selected)
    return tuple(selected)


def query_token_balance_weights(
    query_tokens: tuple[str, ...],
    query_clauses: tuple[str, ...] = (),
) -> Mapping[str, float]:
    weights = {token: QUERY_TOKEN_UNCOVERED_BONUS for token in query_tokens}
    clause_groups = _query_clause_token_groups(query_clauses)
    if len(clause_groups) < 2:
        return weights
    for index, clause in enumerate(clause_groups):
        multiplier = (
            QUERY_TOKEN_PRIMARY_CLAUSE_MULTIPLIER
            if index == 0
            else QUERY_TOKEN_SECONDARY_CLAUSE_MULTIPLIER
        )
        for token in clause:
            if token in weights:
                weights[token] = max(
                    weights[token],
                    QUERY_TOKEN_UNCOVERED_BONUS * multiplier,
                )
    return weights


def exact_query_tokens_for_node(
    node: Node,
    query_tokens: tuple[str, ...],
    *,
    include_query_node: bool = True,
) -> tuple[str, ...]:
    if not include_query_node and node.kind == "query":
        return ()
    node_tokens = set(query_tokens_from_text(_node_exact_text(node)))
    return tuple(token for token in query_tokens if token in node_tokens)


def _node_exact_text(node: Node) -> str:
    exact_fields = [
        node.value,
        str(node.fields.get("symbol") or ""),
        str(node.fields.get("name") or ""),
    ]
    if node.kind in {"owner", "test"}:
        exact_fields.append(str(node.fields.get("path") or ""))
        exact_fields.append(str(node.fields.get("ownerPath") or ""))
    return " ".join(exact_fields)


def _query_clause_token_groups(query_clauses: tuple[str, ...]) -> tuple[tuple[str, ...], ...]:
    return tuple(
        tokens
        for clause in query_clauses
        if isinstance(clause, str)
        and (tokens := query_tokens_from_text(clause))
    )
