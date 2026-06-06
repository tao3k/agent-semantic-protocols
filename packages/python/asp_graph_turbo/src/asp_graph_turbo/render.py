"""Graph-frontier compact rendering for graph turbo results."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .constants import ALGORITHM_ID
from .model import Edge, GraphProfile, GraphResult, Node
from .profiles import frontier_action


def render_compact(result: GraphResult) -> str:
    aliases = _aliases(result.ranked_nodes)
    seed_line = _render_seed_aliases(result.seed_ids, aliases)
    alias_line = _render_aliases(result.ranked_nodes, aliases)
    node_lines = [_render_node(result.profile, node, aliases) for node in result.ranked_nodes]
    edge_lines = [
        *_render_root_edges(result.seed_ids, result.ranked_nodes, aliases),
        *_render_edges(result.selected_edges, aliases),
    ]
    rank_line = ",".join(aliases[node.id] for node in result.ranked_nodes)
    frontier_line = ",".join(
        f"{aliases[entry.node.id]}.{entry.action}" for entry in result.frontier
    )
    score_line = _render_scores(result, aliases)
    lines = [
        (
            f"[graph-frontier] profile={result.profile.name} alg={ALGORITHM_ID} "
            f"seed={seed_line} budget={result.budget}"
        ),
        "legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next",
        alias_line,
        *node_lines,
        *edge_lines,
        f"rank={rank_line}",
        f"frontier={frontier_line}",
        f"scores={score_line}",
        f"profiles={','.join(result.profiles)}",
        f"omit={','.join(result.omit)}",
        f"avoid={','.join(result.avoid)}",
    ]
    return "\n".join(lines) + "\n"


def _aliases(nodes: Iterable[Node]) -> Mapping[str, str]:
    prefixes = {
        "dependency": "D",
        "finding": "F",
        "hot": "H",
        "item": "I",
        "owner": "O",
        "query": "Q",
        "range": "R",
        "symbol": "S",
        "test": "T",
        "window": "W",
    }
    counts: dict[str, int] = {}
    aliases: dict[str, str] = {}
    for node in nodes:
        prefix = prefixes.get(node.kind, "N")
        counts[prefix] = counts.get(prefix, 0) + 1
        aliases[node.id] = prefix if counts[prefix] == 1 else f"{prefix}{counts[prefix]}"
    return aliases


def _render_seed_aliases(seed_ids: Iterable[str], aliases: Mapping[str, str]) -> str:
    seed_aliases = [aliases[node_id] for node_id in seed_ids if node_id in aliases]
    return ",".join(seed_aliases) if seed_aliases else "-"


def _render_aliases(nodes: Iterable[Node], aliases: Mapping[str, str]) -> str:
    node_aliases = [f"{aliases[node.id]}:{node.kind}" for node in nodes]
    return f"aliases={','.join(['G:graph', *node_aliases])}"


def _render_node(profile: GraphProfile, node: Node, aliases: Mapping[str, str]) -> str:
    locator = _node_locator(node)
    action = frontier_action(profile, node) or node.action or "inspect"
    return f"{aliases[node.id]}={node.kind}:{node.role}({node.value}){locator}!{action}"


def _node_locator(node: Node) -> str:
    locator = node.fields.get("locator") or node.fields.get("location")
    if locator is not None:
        return f"@{locator}"
    path = node.fields.get("path")
    start = node.fields.get("startLine") or node.fields.get("start")
    end = node.fields.get("endLine") or node.fields.get("end")
    if path is not None and start is not None and end is not None:
        return f"@{path}:{start}:{end}"
    return ""


def _render_root_edges(
    seed_ids: Iterable[str], nodes: Iterable[Node], aliases: Mapping[str, str]
) -> list[str]:
    node_by_id = {node.id: node for node in nodes}
    targets = [
        f"{aliases[node_id]}:{_root_relation(node_by_id[node_id])}"
        for node_id in seed_ids
        if node_id in aliases and node_id in node_by_id
    ]
    return [f"G>{{{','.join(targets)}}}"] if targets else []


def _root_relation(node: Node) -> str:
    if node.kind == "query":
        return "matches"
    return "selects"


def _render_edges(edges: Iterable[Edge], aliases: Mapping[str, str]) -> list[str]:
    groups: dict[str, list[str]] = {}
    for edge in edges:
        source = aliases.get(edge.source)
        target = aliases.get(edge.target)
        if source is None or target is None:
            continue
        groups.setdefault(source, []).append(f"{target}:{edge.relation}")
    if not groups:
        return []
    return [
        f"{source}>{{{','.join(targets)}}}" for source, targets in sorted(groups.items())
    ]


def _render_scores(result: GraphResult, aliases: Mapping[str, str]) -> str:
    ranked_scores = [
        result.scores[node.id] for node in result.ranked_nodes if node.id in result.scores
    ]
    max_score = max(ranked_scores, default=0.0)
    return ",".join(
        f"{aliases[node.id]}:{_compact_score(result.scores[node.id], max_score)}"
        for node in result.ranked_nodes
        if node.id in result.scores
    )


def _compact_score(score: float, max_score: float) -> str:
    normalized = score / max_score if max_score > 0.0 else 0.0
    return f"{normalized:.2f}"
