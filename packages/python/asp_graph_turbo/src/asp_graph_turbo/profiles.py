"""Graph turbo profile catalog, transition masks, and action lookup."""

from __future__ import annotations

from collections.abc import Iterable

from .model import AllowedTransition, Edge, GraphProfile, Node, TypedGraph
from .policy import NODE_KIND_BONUS_BY_PROFILE

_DEFAULT_FRONTIER_ACTIONS = {
    "assert": "evidence",
    "build": "build",
    "collection": "code",
    "dependency": "deps",
    "dependency-version": "evidence",
    "evidence": "evidence",
    "failure": "failure",
    "field": "code",
    "finding": "owner",
    "hot": "code",
    "api-symbol": "code",
    "import-site": "code",
    "item": "code",
    "key": "evidence",
    "owner": "owner",
    "package": "package",
    "query": "fzf",
    "range": "code",
    "symbol": "code",
    "test": "tests",
    "type": "code",
    "window": "code",
}


def _transitions(items: Iterable[tuple[str, str]]) -> frozenset[AllowedTransition]:
    return frozenset(AllowedTransition(source, target) for source, target in items)


DEFAULT_PROFILES: dict[str, GraphProfile] = {
    "owner-query": GraphProfile(
        name="owner-query",
        allowed_relations=frozenset(
            {
                "matches",
                "selects",
                "repairs",
                "contains",
                "calls",
                "covers",
                "covered_by",
                "has_type",
                "collection_of",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("query", "item"),
                ("query", "field"),
                ("query", "type"),
                ("query", "collection"),
                ("owner", "item"),
                ("owner", "field"),
                ("item", "hot"),
                ("field", "hot"),
                ("field", "type"),
                ("field", "collection"),
                ("type", "collection"),
                ("owner", "test"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["owner-query"],
    ),
    "query-deps": GraphProfile(
        name="query-deps",
        allowed_relations=frozenset(
            {
                "matches",
                "selects",
                "uses",
                "imports",
                "depends_on",
                "version_locked",
                "covers",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("query", "owner"),
                ("owner", "dependency"),
                ("dependency", "dependency-version"),
                ("dependency", "owner"),
                ("owner", "test"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
    ),
    "owner-tests": GraphProfile(
        name="owner-tests",
        allowed_relations=frozenset({"covers", "covered_by", "tests", "selects"}),
        allowed_transitions=_transitions((("owner", "test"), ("test", "owner"))),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
    ),
    "prime": GraphProfile(
        name="prime",
        allowed_relations=frozenset(
            {"matches", "selects", "uses", "imports", "covers"}
        ),
        allowed_transitions=_transitions(
            (
                ("query", "owner"),
                ("owner", "dependency"),
                ("dependency", "owner"),
                ("owner", "test"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["prime"],
    ),
    "read-frontier": GraphProfile(
        name="read-frontier",
        allowed_relations=frozenset({"contains", "split", "selects", "covers"}),
        allowed_transitions=_transitions(
            (
                ("range", "symbol"),
                ("range", "window"),
                ("symbol", "window"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["read-frontier"],
    ),
    "failure-frontier": GraphProfile(
        name="failure-frontier",
        allowed_relations=frozenset(
            {
                "checks",
                "contains",
                "explains",
                "fails",
                "gates",
                "matches",
                "relates",
                "selects",
                "validates",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("failure", "assert"),
                ("failure", "owner"),
                ("failure", "test"),
                ("assert", "evidence"),
                ("assert", "hot"),
                ("assert", "key"),
                ("owner", "hot"),
                ("hot", "evidence"),
                ("hot", "key"),
            )
        ),
        frontier_actions={**_DEFAULT_FRONTIER_ACTIONS, "test": "code"},
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["failure-frontier"],
        max_depth=3,
    ),
    "field-impact": GraphProfile(
        name="field-impact",
        allowed_relations=frozenset(
            {
                "matches",
                "selects",
                "contains",
                "covers",
                "covered_by",
                "has_type",
                "collection_of",
                "calls",
                "relates",
                "validates",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("query", "field"),
                ("owner", "field"),
                ("field", "type"),
                ("field", "collection"),
                ("field", "hot"),
                ("type", "hot"),
                ("collection", "hot"),
                ("hot", "test"),
                ("owner", "test"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["field-impact"],
        max_depth=4,
    ),
    "type-impact": GraphProfile(
        name="type-impact",
        allowed_relations=frozenset(
            {
                "matches",
                "selects",
                "contains",
                "covers",
                "covered_by",
                "has_type",
                "collection_of",
                "calls",
                "relates",
                "validates",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("query", "type"),
                ("field", "type"),
                ("type", "field"),
                ("type", "collection"),
                ("type", "hot"),
                ("collection", "hot"),
                ("hot", "test"),
                ("owner", "test"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["type-impact"],
        max_depth=4,
    ),
    "collection-impact": GraphProfile(
        name="collection-impact",
        allowed_relations=frozenset(
            {
                "matches",
                "selects",
                "contains",
                "covers",
                "covered_by",
                "has_type",
                "collection_of",
                "calls",
                "relates",
                "validates",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("query", "collection"),
                ("field", "collection"),
                ("collection", "field"),
                ("collection", "type"),
                ("collection", "hot"),
                ("type", "hot"),
                ("hot", "test"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["collection-impact"],
        max_depth=4,
    ),
    "failure-evidence": GraphProfile(
        name="failure-evidence",
        allowed_relations=frozenset(
            {
                "checks",
                "collection_of",
                "contains",
                "explains",
                "fails",
                "gates",
                "has_type",
                "matches",
                "relates",
                "selects",
                "validates",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("failure", "assert"),
                ("failure", "test"),
                ("assert", "evidence"),
                ("assert", "hot"),
                ("assert", "field"),
                ("assert", "collection"),
                ("assert", "type"),
                ("field", "type"),
                ("field", "collection"),
                ("hot", "evidence"),
                ("hot", "field"),
                ("hot", "collection"),
                ("hot", "type"),
            )
        ),
        frontier_actions={**_DEFAULT_FRONTIER_ACTIONS, "test": "code"},
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["failure-evidence"],
        max_depth=4,
    ),
    "test-selection": GraphProfile(
        name="test-selection",
        allowed_relations=frozenset(
            {
                "affects",
                "belongs_to",
                "builds",
                "contains",
                "covered_by",
                "covers",
                "matches",
                "packages",
                "selects",
                "targets",
                "tests",
                "validates",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("query", "owner"),
                ("query", "field"),
                ("query", "hot"),
                ("field", "owner"),
                ("field", "package"),
                ("hot", "owner"),
                ("hot", "package"),
                ("field", "test"),
                ("hot", "test"),
                ("owner", "test"),
                ("owner", "package"),
                ("package", "build"),
                ("build", "test"),
                ("package", "test"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["test-selection"],
        max_depth=4,
    ),
    "affected": GraphProfile(
        name="affected",
        allowed_relations=frozenset(
            {
                "affects",
                "belongs_to",
                "builds",
                "contains",
                "covered_by",
                "covers",
                "depends_on",
                "imports",
                "matches",
                "packages",
                "selects",
                "targets",
                "tests",
                "uses",
                "validates",
            }
        ),
        allowed_transitions=_transitions(
            (
                ("query", "owner"),
                ("query", "field"),
                ("field", "owner"),
                ("field", "package"),
                ("hot", "owner"),
                ("hot", "package"),
                ("owner", "package"),
                ("owner", "dependency"),
                ("owner", "test"),
                ("package", "build"),
                ("package", "dependency"),
                ("package", "test"),
                ("build", "test"),
                ("dependency", "package"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["affected"],
        max_depth=4,
    ),
}

_PROFILE_ALIASES = {"read-plan": "read-frontier"}


def resolve_profile(profile: str | GraphProfile) -> GraphProfile:
    if isinstance(profile, GraphProfile):
        return profile
    profile = _PROFILE_ALIASES.get(profile, profile)
    try:
        return DEFAULT_PROFILES[profile]
    except KeyError as error:
        names = ", ".join(sorted(DEFAULT_PROFILES))
        raise ValueError(
            f"unknown graph profile {profile!r}; expected one of {names}"
        ) from error


def frontier_action(profile: GraphProfile, node: Node) -> str | None:
    return node.action or profile.frontier_actions.get(node.kind)


def allowed_oriented_edges(
    graph: TypedGraph, profile: GraphProfile
) -> tuple[tuple[str, str, Edge], ...]:
    oriented: list[tuple[str, str, Edge]] = []
    for edge in graph.edges:
        if edge.weight <= 0.0 or edge.relation not in profile.allowed_relations:
            continue
        source = graph.nodes[edge.source]
        target = graph.nodes[edge.target]
        if AllowedTransition(source.kind, target.kind) in profile.allowed_transitions:
            oriented.append((edge.source, edge.target, edge))
        if AllowedTransition(target.kind, source.kind) in profile.allowed_transitions:
            oriented.append((edge.target, edge.source, edge))
    return tuple(oriented)
