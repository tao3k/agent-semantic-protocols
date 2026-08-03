from __future__ import annotations

import hashlib
import json
import unicodedata
from dataclasses import asdict, dataclass
from typing import Any, Iterable, Mapping, Sequence, TypeAlias


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


@dataclass(frozen=True, slots=True)
class Relation:
    predicate: str
    columns: tuple[str, ...]
    rows: tuple[tuple[Scalar, ...], ...]


@dataclass(frozen=True, slots=True)
class SourceASTBinding:
    source_digest: str
    ast_digest: str
    binding_digest: str


@dataclass(frozen=True, slots=True)
class SemanticTrace:
    gql_binding: SourceASTBinding
    logic_binding: SourceASTBinding | None
    graph_digest_before: str
    graph_digest_after: str
    registry_digest: str
    execution_binding_digest: str
    gql_rows: tuple[tuple[tuple[str, Scalar], ...], ...]
    logic_rows: tuple[tuple[tuple[str, Scalar], ...], ...]
    multiplicity: str
    ordering: str
    trace_digest: str

    def payload_without_digest(self) -> dict[str, Any]:
        payload = asdict(self)
        payload.pop("trace_digest")
        return payload


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def digest(value: Any) -> str:
    encoded = canonical_json(value).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def _tokenize(source: str) -> tuple[Token, ...]:
    if unicodedata.normalize("NFC", source) != source:
        raise SemanticParseError("noncanonical-unicode", 0, "Source must use NFC.")
    tokens: list[Token] = []
    index = 0
    while index < len(source):
        character = source[index]
        if character.isspace():
            index += 1
            continue
        if character == "'":
            index += 1
            value: list[str] = []
            while index < len(source):
                if source[index] == "'":
                    if index + 1 < len(source) and source[index + 1] == "'":
                        value.append("'")
                        index += 2
                        continue
                    index += 1
                    tokens.append(Token("string", "".join(value)))
                    break
                if source[index] == "\n":
                    raise SemanticParseError(
                        "multiline-string-not-admitted",
                        len(tokens),
                        "String may not cross a frame.",
                    )
                value.append(source[index])
                index += 1
            else:
                raise SemanticParseError(
                    "unterminated-string", len(tokens), "String is not terminated."
                )
            continue
        if character.isalpha() or character == "_":
            start = index
            index += 1
            while index < len(source) and (
                source[index].isalnum() or source[index] in "_-"
            ):
                index += 1
            tokens.append(Token("identifier", source[start:index]))
            continue
        if character.isdigit():
            start = index
            index += 1
            while index < len(source) and source[index].isdigit():
                index += 1
            tokens.append(Token("integer", source[start:index]))
            continue
        for symbol in ("->", "?-", ":-"):
            if source.startswith(symbol, index):
                tokens.append(Token("symbol", symbol))
                index += len(symbol)
                break
        else:
            if character not in "()[]:-.,={}":
                raise SemanticParseError(
                    "unknown-token", len(tokens), f"Token {character!r} is not admitted."
                )
            tokens.append(Token("symbol", character))
            index += 1
            continue
        continue
    return tuple(tokens)


class _Cursor:
    def __init__(self, tokens: Sequence[Token]) -> None:
        self.tokens = tokens
        self.index = 0

    def peek(self, value: str | None = None) -> bool:
        if self.index >= len(self.tokens):
            return False
        return value is None or self.tokens[self.index].value.upper() == value.upper()

    def take(self) -> Token:
        if self.index >= len(self.tokens):
            raise SemanticParseError("unexpected-end", self.index, "Unexpected end of query.")
        token = self.tokens[self.index]
        self.index += 1
        return token

    def expect(self, value: str) -> Token:
        token = self.take()
        if token.value.upper() != value.upper():
            raise SemanticParseError(
                "unexpected-token",
                self.index - 1,
                f"Expected {value!r}, observed {token.value!r}.",
            )
        return token

    def identifier(self) -> str:
        token = self.take()
        if token.kind != "identifier":
            raise SemanticParseError(
                "identifier-required", self.index - 1, "Identifier is required."
            )
        return token.value

    def finish(self) -> None:
        if self.index != len(self.tokens):
            raise SemanticParseError(
                "trailing-token", self.index, "Query has an unparsed trailing token."
            )


def _parse_node(cursor: _Cursor) -> NodePattern:
    cursor.expect("(")
    variable = cursor.identifier()
    cursor.expect(":")
    label = cursor.identifier()
    cursor.expect(")")
    return NodePattern(variable=variable, label=label)


def parse_gql(source: str) -> GQLQuery:
    cursor = _Cursor(_tokenize(source))
    cursor.expect("MATCH")
    left = _parse_node(cursor)
    cursor.expect("-")
    cursor.expect("[")
    cursor.expect(":")
    relation = cursor.identifier()
    cursor.expect("]")
    cursor.expect("->")
    right = _parse_node(cursor)
    where: EqualityPredicate | None = None
    if cursor.peek("WHERE"):
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
        where = EqualityPredicate(variable, property_name, value)
    cursor.expect("RETURN")
    returns: list[ReturnBinding] = []
    while True:
        variable = cursor.identifier()
        cursor.expect("AS")
        alias = cursor.identifier()
        returns.append(ReturnBinding(variable, alias))
        if not cursor.peek(","):
            break
        cursor.take()
    cursor.finish()
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
        raise SemanticParseError("duplicate-return-alias", 0, "RETURN aliases must be unique.")
    return GQLQuery(
        left=left,
        edge=EdgePattern(relation),
        right=right,
        where=where,
        returns=tuple(returns),
    )


def _logic_term(token: Token) -> LogicTerm:
    if token.kind == "string":
        return LogicTerm(token.value, variable=False)
    if token.kind == "integer":
        return LogicTerm(int(token.value), variable=False)
    if token.kind != "identifier":
        raise SemanticParseError("logic-term-required", 0, "Logic term is required.")
    if token.value.upper() == "NULL":
        raise SemanticParseError("null-not-in-profile", 0, "NULL semantics are not in v0.1.")
    return LogicTerm(token.value, variable=token.value[0].isupper())


def parse_logic(source: str, *, registered_predicates: Iterable[str]) -> LogicQuery:
    cursor = _Cursor(_tokenize(source))
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
        {
            "sourceDigest": source_digest,
            "astDigest": ast_digest,
            "profile": ast.profile,
        }
    )
    return SourceASTBinding(source_digest, ast_digest, binding_digest)


def graph_digest(graph: PropertyGraph) -> str:
    return digest(
        {
            "nodes": [
                asdict(node) for node in sorted(graph.nodes, key=lambda item: item.node_id)
            ],
            "edges": [
                asdict(edge) for edge in sorted(graph.edges, key=lambda item: item.edge_id)
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
        row = tuple(
            (binding.alias, bindings[binding.variable].node_id)
            for binding in query.returns
        )
        rows.append(row)
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
                "logic-relation-arity", 0, f"Relation {atom.predicate!r} has another arity."
            )
        for term in atom.terms:
            if term.variable and str(term.value) not in variable_order:
                variable_order.append(str(term.value))
        next_environments: list[dict[str, Scalar]] = []
        for environment in environments:
            for row in relation.rows:
                candidate = dict(environment)
                compatible = True
                for term, observed in zip(atom.terms, row, strict=True):
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
        environments = next_environments
    rows = [
        tuple((variable, environment[variable]) for variable in variable_order)
        for environment in environments
    ]
    return tuple(sorted(rows, key=canonical_json))


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
    aliases = tuple(binding.alias for binding in gql.returns)
    gql_relation = Relation(
        predicate="gql_candidate",
        columns=aliases,
        rows=tuple(tuple(value for _, value in row) for row in gql_rows),
    )
    registry = tuple(relations) + (gql_relation,)
    registry_digest = relation_registry_digest(registry)
    logic_binding: SourceASTBinding | None = None
    logic_rows: tuple[tuple[tuple[str, Scalar], ...], ...] = ()
    if logic_source is not None:
        logic = parse_logic(
            logic_source,
            registered_predicates=(relation.predicate for relation in registry),
        )
        logic_binding = source_ast_binding(logic_source, logic)
        logic_rows = evaluate_logic(logic, registry)
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


def trace_is_admitted(trace: SemanticTrace) -> bool:
    return (
        trace.graph_digest_before == trace.graph_digest_after
        and trace.trace_digest == digest(trace.payload_without_digest())
        and trace.multiplicity == "bag-by-witness-edge"
        and trace.ordering == "canonical-row-json"
    )
