# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Own the typed query models shared by polyglot search semantics."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TypeAlias


Scalar: TypeAlias = str | int | bool


class SemanticParseError(ValueError):
    def __init__(self, code: str, token_index: int, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.token_index = token_index


@dataclass(frozen=True, slots=True)
class Token:
    kind: str
    value: str


@dataclass(frozen=True, slots=True)
class NodePattern:
    variable: str
    label: str


@dataclass(frozen=True, slots=True)
class EdgePattern:
    relation: str
    direction: str = "outgoing"


@dataclass(frozen=True, slots=True)
class EqualityPredicate:
    variable: str
    property_name: str
    value: Scalar


@dataclass(frozen=True, slots=True)
class ReturnBinding:
    variable: str
    alias: str


@dataclass(frozen=True, slots=True)
class GQLQuery:
    left: NodePattern
    edge: EdgePattern
    right: NodePattern
    where: EqualityPredicate | None
    returns: tuple[ReturnBinding, ...]
    profile: str = "asp-gql-core:0.1-one-hop"


@dataclass(frozen=True, slots=True)
class LogicTerm:
    value: Scalar
    variable: bool


@dataclass(frozen=True, slots=True)
class LogicAtom:
    predicate: str
    terms: tuple[LogicTerm, ...]


@dataclass(frozen=True, slots=True)
class LogicQuery:
    atoms: tuple[LogicAtom, ...]
    profile: str = "asp-logic-query-core:0.1-positive-conjunction"


@dataclass(frozen=True, slots=True)
class GraphNode:
    node_id: str
    labels: tuple[str, ...]
    properties: tuple[tuple[str, Scalar], ...]

    def property(self, name: str) -> Scalar | None:
        return dict(self.properties).get(name)


@dataclass(frozen=True, slots=True)
class GraphEdge:
    edge_id: str
    source_id: str
    target_id: str
    relation: str


@dataclass(frozen=True, slots=True)
class PropertyGraph:
    nodes: tuple[GraphNode, ...]
    edges: tuple[GraphEdge, ...]
