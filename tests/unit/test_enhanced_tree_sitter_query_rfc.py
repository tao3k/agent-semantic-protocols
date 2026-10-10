# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Contract checks for the resident enhanced Tree-sitter Query design."""

from pathlib import Path


_ROOT = Path(__file__).resolve().parents[2]
_PROJECTION_RFC = _ROOT / "docs/10-19-rfcs/10.06-agent-search-projection.org"
_PLAYBOOK_RFC = (
    _ROOT
    / "docs/10-19-rfcs/10.06-agent-search-projection"
    / "10.06.15-search-query-playbook-contract.org"
)


def test_enhanced_query_is_additive_tree_sitter_query() -> None:
    projection = _PROJECTION_RFC.read_text(encoding="utf-8")
    playbook = _PLAYBOOK_RFC.read_text(encoding="utf-8")

    required = [
        "=asp.enhanced-tree-sitter-query.v1=",
        "=#asp-*=",
        "does not add a =(where ...)= wrapper",
        "Tree-sitter Query itself",
        "=mrr.enhanced-tree-sitter-query.v1=",
        "after Scheme string decoding it is parsed as Tree-sitter Query",
    ]

    assert [term for term in required if term not in projection + playbook] == []


def test_v1_fact_and_operator_surface_is_closed() -> None:
    text = _PLAYBOOK_RFC.read_text(encoding="utf-8")

    required = [
        "=kind=, =name=, =selector=, =byte-range=, =scopes=, =queryKeys=",
        "#asp-eq?",
        "#asp-any-match?",
        "#asp-range?",
        "#asp-related?",
        "#asp-select!",
        "=ResidentSyntaxQueryPlanV1=",
        "=mrr-enhanced-tree-sitter-query-operator-table-v1=",
        "unsigned 64-bit little-endian",
        "RFC 8785 JCS bytes",
        "explicit typed =true= condition",
        "standard text predicates as =queryKeys= predicates",
        "silently discard an unknown predicate",
        "Capture-to-capture",
    ]

    assert [term for term in required if term not in text] == []
