# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Stable public facade for polyglot search semantics."""

from asp_proofs._polyglot_search_semantics_evaluator import (
    evaluate_gql,
    evaluate_logic,
    graph_digest,
    relation_registry_digest,
)
from asp_proofs._polyglot_search_semantics_execution import (
    execute_semantics,
    trace_is_admitted,
)
from asp_proofs._polyglot_search_semantics_model import (
    EdgePattern,
    EqualityPredicate,
    GQLQuery,
    GraphEdge,
    GraphNode,
    LogicAtom,
    LogicQuery,
    LogicTerm,
    NodePattern,
    PropertyGraph,
    ReturnBinding,
    Scalar,
    SemanticParseError,
    Token,
)
from asp_proofs._polyglot_search_semantics_parser import (
    parse_gql,
    parse_logic,
    source_ast_binding,
)
from asp_proofs._polyglot_search_semantics_types import (
    Relation,
    SemanticTrace,
    SourceASTBinding,
    canonical_json,
    digest,
)

__all__ = [
    "EdgePattern",
    "EqualityPredicate",
    "GQLQuery",
    "GraphEdge",
    "GraphNode",
    "LogicAtom",
    "LogicQuery",
    "LogicTerm",
    "NodePattern",
    "PropertyGraph",
    "Relation",
    "ReturnBinding",
    "Scalar",
    "SemanticParseError",
    "SemanticTrace",
    "SourceASTBinding",
    "Token",
    "canonical_json",
    "digest",
    "evaluate_gql",
    "evaluate_logic",
    "execute_semantics",
    "graph_digest",
    "parse_gql",
    "parse_logic",
    "relation_registry_digest",
    "source_ast_binding",
    "trace_is_admitted",
]
