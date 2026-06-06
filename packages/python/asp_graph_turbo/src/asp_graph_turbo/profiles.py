"""Graph turbo profile catalog, transition masks, and action lookup."""

from __future__ import annotations

from collections.abc import Iterable

from .model import AllowedTransition, Edge, GraphProfile, Node, TypedGraph
from .policy import NODE_KIND_BONUS_BY_PROFILE

_DEFAULT_FRONTIER_ACTIONS = {
    "dependency": "deps",
    "finding": "owner",
    "hot": "code",
    "item": "code",
    "owner": "owner",
    "query": "fzf",
    "range": "code",
    "symbol": "code",
    "test": "tests",
    "window": "code",
}


def _transitions(items: Iterable[tuple[str, str]]) -> frozenset[AllowedTransition]:
    return frozenset(AllowedTransition(source, target) for source, target in items)


DEFAULT_PROFILES: dict[str, GraphProfile] = {
    "owner-query": GraphProfile(
        name="owner-query",
        allowed_relations=frozenset(
            {"matches", "selects", "repairs", "contains", "calls", "covers", "covered_by"}
        ),
        allowed_transitions=_transitions(
            (
                ("query", "item"),
                ("owner", "item"),
                ("item", "hot"),
                ("owner", "test"),
            )
        ),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
        kind_bonus=NODE_KIND_BONUS_BY_PROFILE["owner-query"],
    ),
    "query-deps": GraphProfile(
        name="query-deps",
        allowed_relations=frozenset(
            {"matches", "selects", "uses", "imports", "depends_on", "covers"}
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
    ),
    "owner-tests": GraphProfile(
        name="owner-tests",
        allowed_relations=frozenset({"covers", "covered_by", "tests", "selects"}),
        allowed_transitions=_transitions((("owner", "test"), ("test", "owner"))),
        frontier_actions=_DEFAULT_FRONTIER_ACTIONS,
    ),
    "prime": GraphProfile(
        name="prime",
        allowed_relations=frozenset({"matches", "selects", "uses", "imports", "covers"}),
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
        raise ValueError(f"unknown graph profile {profile!r}; expected one of {names}") from error


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
