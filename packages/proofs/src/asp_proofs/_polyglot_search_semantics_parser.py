# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Parsers for the admitted one-hop GQL and conjunctive logic profiles."""

from __future__ import annotations

import unicodedata
from collections.abc import Iterable
from dataclasses import asdict

from asp_proofs._polyglot_search_semantics_lexer import Cursor, tokenize
from asp_proofs._polyglot_search_semantics_model import (
    EdgePattern,
    EqualityPredicate,
    GQLQuery,
    LogicAtom,
    LogicQuery,
    LogicTerm,
    NodePattern,
    ReturnBinding,
    Scalar,
    SemanticParseError,
    Token,
)
from asp_proofs._polyglot_search_semantics_types import SourceASTBinding, digest


def _parse_node(cursor: Cursor) -> NodePattern:
    cursor.expect("(")
    variable = cursor.identifier()
    cursor.expect(":")
    label = cursor.identifier()
    cursor.expect(")")
    return NodePattern(variable=variable, label=label)


def parse_gql(source: str) -> GQLQuery:
    cursor = Cursor(tokenize(source))
    cursor.expect("MATCH")
    left = _parse_node(cursor)
    cursor.expect("-")
    cursor.expect("[")
    cursor.expect(":")
    relation = cursor.identifier()
    cursor.expect("]")
    cursor.expect("->")
    right = _parse_node(cursor)
    where = _parse_where(cursor)
    cursor.expect("RETURN")
    returns = _parse_returns(cursor)
    cursor.finish()
    _validate_gql_bindings(left, right, where, returns)
    return GQLQuery(
        left=left,
        edge=EdgePattern(relation),
        right=right,
        where=where,
        returns=returns,
    )


def _parse_where(cursor: Cursor) -> EqualityPredicate | None:
    if not cursor.peek("WHERE"):
        return None
    cursor.take()
    variable = cursor.identifier()
    cursor.expect(".")
    property_name = cursor.identifier()
    cursor.expect("=")
    value_token = cursor.take()
    if value_token.kind == "string":
        value: Scalar = value_token.value
    elif value_token.kind == "integer":
        value = int(value_token.value)
    elif value_token.kind == "identifier" and value_token.value in {"true", "false"}:
        value = value_token.value == "true"
    else:
        raise SemanticParseError(
            "typed-literal-required",
            cursor.index - 1,
            "WHERE equality requires string, integer, or Boolean literal.",
        )
    return EqualityPredicate(variable, property_name, value)


def _parse_returns(cursor: Cursor) -> tuple[ReturnBinding, ...]:
    returns: list[ReturnBinding] = []
    while True:
        variable = cursor.identifier()
        cursor.expect("AS")
        returns.append(ReturnBinding(variable, cursor.identifier()))
        if not cursor.peek(","):
            return tuple(returns)
        cursor.take()


def _validate_gql_bindings(
    left: NodePattern,
    right: NodePattern,
    where: EqualityPredicate | None,
    returns: tuple[ReturnBinding, ...],
) -> None:
    bound_variables = {left.variable, right.variable}
    if left.variable == right.variable:
        raise SemanticParseError(
            "variable-shadowing", 0, "One-hop endpoint variables must be distinct."
        )
    if where is not None and where.variable not in bound_variables:
        raise SemanticParseError(
            "unbound-where-variable", 0, "WHERE variable is not bound by MATCH."
        )
    if any(binding.variable not in bound_variables for binding in returns):
        raise SemanticParseError(
            "unbound-return-variable", 0, "RETURN variable is not bound by MATCH."
        )
    aliases = [binding.alias for binding in returns]
    if len(aliases) != len(set(aliases)):
        raise SemanticParseError(
            "duplicate-return-alias", 0, "RETURN aliases must be unique."
        )


def _logic_term(token: Token) -> LogicTerm:
    if token.kind == "string":
        return LogicTerm(token.value, variable=False)
    if token.kind == "integer":
        return LogicTerm(int(token.value), variable=False)
    if token.kind != "identifier":
        raise SemanticParseError("logic-term-required", 0, "Logic term is required.")
    if token.value.upper() == "NULL":
        raise SemanticParseError(
            "null-not-in-profile", 0, "NULL semantics are not in v0.1."
        )
    return LogicTerm(token.value, variable=token.value[0].isupper())


def parse_logic(source: str, *, registered_predicates: Iterable[str]) -> LogicQuery:
    cursor = Cursor(tokenize(source))
    cursor.expect("?-")
    atoms: list[LogicAtom] = []
    registered = frozenset(registered_predicates)
    while True:
        predicate = cursor.identifier()
        if predicate not in registered:
            raise SemanticParseError(
                "logic-predicate-unregistered",
                cursor.index - 1,
                f"Predicate {predicate!r} is not registered.",
            )
        cursor.expect("(")
        terms: list[LogicTerm] = []
        while True:
            terms.append(_logic_term(cursor.take()))
            if not cursor.peek(","):
                break
            cursor.take()
        cursor.expect(")")
        atoms.append(LogicAtom(predicate, tuple(terms)))
        if not cursor.peek(","):
            break
        cursor.take()
    cursor.expect(".")
    cursor.finish()
    return LogicQuery(tuple(atoms))


def source_ast_binding(source: str, ast: GQLQuery | LogicQuery) -> SourceASTBinding:
    normalized = unicodedata.normalize("NFC", source)
    source_digest = digest({"source": normalized})
    ast_digest = digest(asdict(ast))
    binding_digest = digest(
        {"sourceDigest": source_digest, "astDigest": ast_digest, "profile": ast.profile}
    )
    return SourceASTBinding(source_digest, ast_digest, binding_digest)
