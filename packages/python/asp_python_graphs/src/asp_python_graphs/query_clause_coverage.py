"""Query clause coverage scoring for graph-turbo."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .model import Node
from .query_tokens import query_tokens_from_text

CLAUSE_COVERAGE_BONUS = 0.30
MATCH_TEXT_CLAUSE_MIN_TOKENS = 2


def query_clause_coverage_adjustment(
    *,
    profile_name: str,
    query_clauses: Iterable[str],
    node: Node,
) -> float:
    if profile_name != "owner-query" or node.kind == "query":
        return 0.0
    clauses = _clause_token_groups(query_clauses)
    if len(clauses) < 2:
        return 0.0
    covered_count = sum(1 for tokens in clauses if _node_covers_clause(node, tokens))
    if covered_count < 2:
        return 0.0
    return (covered_count - 1) * CLAUSE_COVERAGE_BONUS


def _clause_token_groups(query_clauses: Iterable[str]) -> tuple[tuple[str, ...], ...]:
    clauses = tuple(query_tokens_from_text(clause) for clause in query_clauses)
    clauses = tuple(tokens for tokens in clauses if tokens)
    if len(clauses) != 1:
        return clauses
    return _split_single_clause_tokens(clauses[0])


def _split_single_clause_tokens(tokens: tuple[str, ...]) -> tuple[tuple[str, ...], ...]:
    if len(tokens) < 6:
        return (tokens,)
    midpoint = max(4, (len(tokens) + 1) // 2)
    return (tokens[:midpoint], tokens[midpoint:])


def _node_covers_clause(node: Node, clause_tokens: tuple[str, ...]) -> bool:
    exact_semantic_tokens = set(query_tokens_from_text(_node_exact_semantic_text(node)))
    package_tokens = tuple(
        token
        for token in clause_tokens
        if "_" in token and token not in exact_semantic_tokens
    )
    if package_tokens and not any(
        token in _node_path_text(node) for token in package_tokens
    ):
        return False
    plain_tokens = tuple(token for token in clause_tokens if "_" not in token)
    if not plain_tokens:
        return True
    if any(token in exact_semantic_tokens for token in plain_tokens):
        return True
    match_text_tokens = set(query_tokens_from_text(_node_semantic_text(node)))
    matched_count = sum(1 for token in plain_tokens if token in match_text_tokens)
    return matched_count >= min(MATCH_TEXT_CLAUSE_MIN_TOKENS, len(plain_tokens))


def _node_exact_semantic_text(node: Node) -> str:
    parts = [
        node.value,
        str(node.fields.get("symbol") or ""),
        str(node.fields.get("name") or ""),
        _node_fields_text(node),
        _semantic_alias_text(node),
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
