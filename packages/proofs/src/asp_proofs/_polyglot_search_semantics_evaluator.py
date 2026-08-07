"""Deterministic evaluators for admitted GQL and logic queries."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import asdict

from asp_proofs._polyglot_search_semantics_model import (
    GQLQuery,
    LogicQuery,
    PropertyGraph,
    Scalar,
    SemanticParseError,
)
from asp_proofs._polyglot_search_semantics_types import (
    Relation,
    canonical_json,
    digest,
)


def graph_digest(graph: PropertyGraph) -> str:
    return digest(
        {
            "nodes": [
                asdict(node)
                for node in sorted(graph.nodes, key=lambda item: item.node_id)
            ],
            "edges": [
                asdict(edge)
                for edge in sorted(graph.edges, key=lambda item: item.edge_id)
            ],
        }
    )


def relation_registry_digest(relations: Sequence[Relation]) -> str:
    canonical_relations = [
        {
            "predicate": relation.predicate,
            "columns": relation.columns,
            "rows": sorted(relation.rows, key=canonical_json),
        }
        for relation in sorted(relations, key=lambda item: item.predicate)
    ]
    return digest(canonical_relations)


def evaluate_gql(
    query: GQLQuery, graph: PropertyGraph
) -> tuple[tuple[tuple[str, Scalar], ...], ...]:
    nodes = {node.node_id: node for node in graph.nodes}
    rows: list[tuple[tuple[str, Scalar], ...]] = []
    for edge in sorted(graph.edges, key=lambda item: item.edge_id):
        if edge.relation != query.edge.relation:
            continue
        left = nodes.get(edge.source_id)
        right = nodes.get(edge.target_id)
        if left is None or right is None:
            continue
        if query.left.label not in left.labels or query.right.label not in right.labels:
            continue
        bindings = {query.left.variable: left, query.right.variable: right}
        if query.where is not None:
            subject = bindings[query.where.variable]
            observed = subject.property(query.where.property_name)
            if observed is None or type(observed) is not type(query.where.value):
                continue
            if observed != query.where.value:
                continue
        rows.append(
            tuple(
                (binding.alias, bindings[binding.variable].node_id)
                for binding in query.returns
            )
        )
    return tuple(sorted(rows, key=canonical_json))


def evaluate_logic(
    query: LogicQuery, relations: Sequence[Relation]
) -> tuple[tuple[tuple[str, Scalar], ...], ...]:
    registry = {relation.predicate: relation for relation in relations}
    environments: list[dict[str, Scalar]] = [{}]
    variable_order: list[str] = []
    for atom in query.atoms:
        relation = registry.get(atom.predicate)
        if relation is None:
            raise SemanticParseError(
                "logic-relation-missing", 0, f"Relation {atom.predicate!r} is missing."
            )
        if len(relation.columns) != len(atom.terms):
            raise SemanticParseError(
                "logic-relation-arity",
                0,
                f"Relation {atom.predicate!r} has another arity.",
            )
        for term in atom.terms:
            if term.variable and str(term.value) not in variable_order:
                variable_order.append(str(term.value))
        environments = _join_atom(environments, atom.terms, relation)
    rows = [
        tuple((variable, environment[variable]) for variable in variable_order)
        for environment in environments
    ]
    return tuple(sorted(rows, key=canonical_json))


def _join_atom(environments, terms, relation):
    next_environments: list[dict[str, Scalar]] = []
    for environment in environments:
        for row in relation.rows:
            candidate = dict(environment)
            compatible = True
            for term, observed in zip(terms, row, strict=True):
                if term.variable:
                    variable = str(term.value)
                    if variable in candidate and (
                        type(candidate[variable]) is not type(observed)
                        or candidate[variable] != observed
                    ):
                        compatible = False
                        break
                    candidate[variable] = observed
                elif type(term.value) is not type(observed) or term.value != observed:
                    compatible = False
                    break
            if compatible:
                next_environments.append(candidate)
    return next_environments
