# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Execution trace construction for polyglot search semantics."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import asdict

from asp_proofs._polyglot_search_semantics_evaluator import (
    evaluate_gql,
    evaluate_logic,
    graph_digest,
    relation_registry_digest,
)
from asp_proofs._polyglot_search_semantics_model import PropertyGraph, Scalar
from asp_proofs._polyglot_search_semantics_parser import (
    parse_gql,
    parse_logic,
    source_ast_binding,
)
from asp_proofs._polyglot_search_semantics_types import (
    Relation,
    SemanticTrace,
    SourceASTBinding,
    digest,
)


def execute_semantics(
    *,
    gql_source: str,
    graph: PropertyGraph,
    logic_source: str | None = None,
    relations: Sequence[Relation] = (),
) -> SemanticTrace:
    gql = parse_gql(gql_source)
    gql_binding = source_ast_binding(gql_source, gql)
    before = graph_digest(graph)
    gql_rows = evaluate_gql(gql, graph)
    gql_relation = Relation(
        predicate="gql_candidate",
        columns=tuple(binding.alias for binding in gql.returns),
        rows=tuple(tuple(value for _, value in row) for row in gql_rows),
    )
    registry = tuple(relations) + (gql_relation,)
    registry_digest = relation_registry_digest(registry)
    logic_binding, logic_rows = _execute_logic(logic_source, registry)
    after = graph_digest(graph)
    execution_binding_digest = digest(
        {
            "gqlBindingDigest": gql_binding.binding_digest,
            "logicBindingDigest": (
                logic_binding.binding_digest if logic_binding is not None else None
            ),
            "graphDigest": before,
            "registryDigest": registry_digest,
            "multiplicity": "bag-by-witness-edge",
            "ordering": "canonical-row-json",
        }
    )
    payload = {
        "gql_binding": asdict(gql_binding),
        "logic_binding": asdict(logic_binding) if logic_binding is not None else None,
        "graph_digest_before": before,
        "graph_digest_after": after,
        "registry_digest": registry_digest,
        "execution_binding_digest": execution_binding_digest,
        "gql_rows": gql_rows,
        "logic_rows": logic_rows,
        "multiplicity": "bag-by-witness-edge",
        "ordering": "canonical-row-json",
    }
    return SemanticTrace(
        gql_binding=gql_binding,
        logic_binding=logic_binding,
        graph_digest_before=before,
        graph_digest_after=after,
        registry_digest=registry_digest,
        execution_binding_digest=execution_binding_digest,
        gql_rows=gql_rows,
        logic_rows=logic_rows,
        multiplicity="bag-by-witness-edge",
        ordering="canonical-row-json",
        trace_digest=digest(payload),
    )


def _execute_logic(
    logic_source: str | None, relations: tuple[Relation, ...]
) -> tuple[
    SourceASTBinding | None,
    tuple[tuple[tuple[str, Scalar], ...], ...],
]:
    if logic_source is None:
        return None, ()
    logic = parse_logic(
        logic_source,
        registered_predicates=(relation.predicate for relation in relations),
    )
    return source_ast_binding(logic_source, logic), evaluate_logic(logic, relations)


def trace_is_admitted(trace: SemanticTrace) -> bool:
    return (
        trace.graph_digest_before == trace.graph_digest_after
        and trace.trace_digest == digest(trace.payload_without_digest())
        and trace.multiplicity == "bag-by-witness-edge"
        and trace.ordering == "canonical-row-json"
    )
