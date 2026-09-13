# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

from dataclasses import replace

import pytest

from asp_proofs.polyglot_search_semantics import (
    GraphEdge,
    GraphNode,
    PropertyGraph,
    Relation,
    SemanticParseError,
    evaluate_gql,
    execute_semantics,
    parse_gql,
    source_ast_binding,
    trace_is_admitted,
)


GQL_SOURCE = (
    "MATCH (f:Callable)-[:OWNED_BY]->(o:Owner) "
    "WHERE f.name = 'open_runtime_project_proxy' "
    "RETURN f AS function, o AS candidate"
)
LOGIC_SOURCE = (
    "?- gql_candidate(Function, Candidate), covers(Candidate, Obligation)."
)


def example_graph() -> PropertyGraph:
    return PropertyGraph(
        nodes=(
            GraphNode(
                "function:1",
                ("Callable",),
                (("name", "open_runtime_project_proxy"), ("version", 1)),
            ),
            GraphNode("owner:1", ("Owner",), (("name", "runtime"),)),
            GraphNode("owner:2", ("Owner",), (("name", "fallback"),)),
            GraphNode("other:1", ("Other",), ()),
        ),
        edges=(
            GraphEdge("edge:2", "function:1", "owner:1", "OWNED_BY"),
            GraphEdge("edge:1", "function:1", "owner:1", "OWNED_BY"),
            GraphEdge("edge:3", "function:1", "owner:2", "OWNED_BY"),
            GraphEdge("edge:4", "owner:1", "other:1", "OWNED_BY"),
        ),
    )


def covers_relation() -> Relation:
    return Relation(
        predicate="covers",
        columns=("candidate", "obligation"),
        rows=(("owner:1", "tests"), ("owner:2", "docs")),
    )


def test_canonical_ast_binds_exact_source() -> None:
    ast = parse_gql(GQL_SOURCE)
    binding = source_ast_binding(GQL_SOURCE, ast)
    whitespace_variant = "  " + GQL_SOURCE
    variant_binding = source_ast_binding(whitespace_variant, parse_gql(whitespace_variant))

    assert ast == parse_gql(GQL_SOURCE)
    assert binding.ast_digest == variant_binding.ast_digest
    assert binding.source_digest != variant_binding.source_digest
    assert binding.binding_digest != variant_binding.binding_digest


def test_one_hop_gql_preserves_parallel_edge_multiplicity() -> None:
    rows = evaluate_gql(parse_gql(GQL_SOURCE), example_graph())

    assert rows == (
        (("function", "function:1"), ("candidate", "owner:1")),
        (("function", "function:1"), ("candidate", "owner:1")),
        (("function", "function:1"), ("candidate", "owner:2")),
    )


def test_where_equality_is_typed_without_coercion() -> None:
    string_query = parse_gql(
        "MATCH (f:Callable)-[:OWNED_BY]->(o:Owner) "
        "WHERE f.version = '1' RETURN o AS candidate"
    )
    integer_query = parse_gql(
        "MATCH (f:Callable)-[:OWNED_BY]->(o:Owner) "
        "WHERE f.version = 1 RETURN o AS candidate"
    )

    assert evaluate_gql(string_query, example_graph()) == ()
    assert len(evaluate_gql(integer_query, example_graph())) == 3


def test_return_aliases_define_the_only_output_columns() -> None:
    query = parse_gql(
        "MATCH (f:Callable)-[:OWNED_BY]->(o:Owner) RETURN o AS selected"
    )
    rows = evaluate_gql(query, example_graph())

    assert rows[0] == (("selected", "owner:1"),)
    assert all(tuple(name for name, _ in row) == ("selected",) for row in rows)


def test_unbound_return_and_alias_shadowing_are_rejected() -> None:
    with pytest.raises(SemanticParseError, match="not bound") as unbound:
        parse_gql(
            "MATCH (f:Callable)-[:OWNED_BY]->(o:Owner) RETURN x AS candidate"
        )
    assert unbound.value.code == "unbound-return-variable"

    with pytest.raises(SemanticParseError, match="unique") as duplicate:
        parse_gql(
            "MATCH (f:Callable)-[:OWNED_BY]->(o:Owner) "
            "RETURN f AS item, o AS item"
        )
    assert duplicate.value.code == "duplicate-return-alias"


def test_conjunctive_logic_join_preserves_shared_binding() -> None:
    trace = execute_semantics(
        gql_source=GQL_SOURCE,
        logic_source=LOGIC_SOURCE,
        graph=example_graph(),
        relations=(covers_relation(),),
    )

    assert trace.logic_rows == (
        (("Function", "function:1"), ("Candidate", "owner:1"), ("Obligation", "tests")),
        (("Function", "function:1"), ("Candidate", "owner:1"), ("Obligation", "tests")),
        (("Function", "function:1"), ("Candidate", "owner:2"), ("Obligation", "docs")),
    )


def test_semantic_trace_is_read_only_and_admitted() -> None:
    trace = execute_semantics(
        gql_source=GQL_SOURCE,
        logic_source=LOGIC_SOURCE,
        graph=example_graph(),
        relations=(covers_relation(),),
    )

    assert trace.graph_digest_before == trace.graph_digest_after
    assert trace_is_admitted(trace)


def test_input_order_does_not_change_trace() -> None:
    graph = example_graph()
    reversed_graph = PropertyGraph(
        nodes=tuple(reversed(graph.nodes)), edges=tuple(reversed(graph.edges))
    )
    normal = execute_semantics(
        gql_source=GQL_SOURCE,
        logic_source=LOGIC_SOURCE,
        graph=graph,
        relations=(covers_relation(),),
    )
    reversed_trace = execute_semantics(
        gql_source=GQL_SOURCE,
        logic_source=LOGIC_SOURCE,
        graph=reversed_graph,
        relations=(covers_relation(),),
    )

    assert normal == reversed_trace


def test_null_semantics_are_explicitly_outside_profile() -> None:
    with pytest.raises(SemanticParseError) as error:
        parse_gql(
            "MATCH (f:Callable)-[:OWNED_BY]->(o:Owner) "
            "WHERE f.name = NULL RETURN o AS candidate"
        )

    assert error.value.code == "typed-literal-required"


def test_tampered_trace_digest_is_rejected() -> None:
    trace = execute_semantics(
        gql_source=GQL_SOURCE,
        logic_source=LOGIC_SOURCE,
        graph=example_graph(),
        relations=(covers_relation(),),
    )
    tampered = replace(trace, trace_digest="sha256:" + "0" * 64)

    assert not trace_is_admitted(tampered)
