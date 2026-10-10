# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""GQL and logic section-profile validation."""

from __future__ import annotations

from typing import Any

from asp_proofs._polyglot_search_conformance_lexer import lex_tokens
from asp_proofs._polyglot_search_conformance_model import Violation

GQL_FORBIDDEN_KEYWORDS = {"CALL", "CREATE", "DELETE", "DROP", "INSERT", "REMOVE", "SET"}
GQL_ALLOWED_CLAUSES = ("MATCH", "WHERE", "RETURN")


def parse_gql(source: str) -> tuple[dict[str, Any], list[Violation]]:
    tokens, violations = lex_tokens(source, path="/GQL")
    keywords = [token.upper() for token in tokens if token and token[0].isalpha()]
    forbidden = sorted(set(keywords) & GQL_FORBIDDEN_KEYWORDS)
    if forbidden:
        violations.append(
            Violation(
                "gql-write-or-procedure-not-admitted",
                "/GQL",
                f"Forbidden GQL keywords: {', '.join(forbidden)}.",
            )
        )
    positions = {
        clause: keywords.index(clause)
        for clause in GQL_ALLOWED_CLAUSES
        if clause in keywords
    }
    if "MATCH" not in positions or "RETURN" not in positions:
        violations.append(
            Violation("gql-clause-missing", "/GQL", "MATCH and RETURN are required.")
        )
    elif positions["MATCH"] > positions["RETURN"]:
        violations.append(
            Violation("gql-clause-order", "/GQL", "MATCH must precede RETURN.")
        )
    if "WHERE" in positions and not (
        positions.get("MATCH", -1)
        < positions["WHERE"]
        < positions.get("RETURN", len(keywords))
    ):
        violations.append(
            Violation(
                "gql-clause-order",
                "/GQL/WHERE",
                "WHERE must be between MATCH and RETURN.",
            )
        )
    if "*" in tokens:
        violations.append(
            Violation(
                "gql-unbounded-path", "/GQL", "Unbounded path syntax is not admitted."
            )
        )
    clauses = [clause.lower() for clause in GQL_ALLOWED_CLAUSES if clause in positions]
    if "{" in tokens and "}" in tokens:
        clauses.append("bounded-path")
    return {
        "profile": "asp-gql-core:0.1",
        "source": source,
        "clauses": clauses,
    }, violations


def parse_logic(
    source: str, registered: frozenset[str]
) -> tuple[dict[str, Any], list[Violation]]:
    tokens, violations = lex_tokens(source, path="/LOGIC")
    if not tokens or tokens[0] != "?-":
        violations.append(
            Violation(
                "logic-query-goal-required", "/LOGIC", "Logic query must begin with ?-."
            )
        )
    if ":-" in tokens:
        violations.append(
            Violation("logic-rule-injection", "/LOGIC", "Rule heads are not admitted.")
        )
    predicates = [
        token
        for index, token in enumerate(tokens[:-1])
        if token
        and (token[0].isalpha() or token[0] == "_")
        and tokens[index + 1] == "("
        and token.lower() != "not"
    ]
    for predicate in sorted(set(predicates) - registered):
        violations.append(
            Violation(
                "logic-predicate-unregistered",
                "/LOGIC",
                f"Predicate {predicate!r} is not registered.",
            )
        )
    forms = ["query-goal", "registered-predicate"]
    if "," in tokens:
        forms.append("conjunction")
    if any(token.lower() == "not" for token in tokens):
        forms.append("safe-negation")
    return {
        "profile": "asp-logic-query-core:0.1",
        "source": source,
        "forms": forms,
        "predicates": predicates,
    }, violations
