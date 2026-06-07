"""Graph-frontier compact rendering for graph turbo results."""

from __future__ import annotations

import re
from collections.abc import Iterable, Mapping

from .constants import ALGORITHM_ID
from .model import Edge, GraphProfile, GraphResult, Node
from .profiles import frontier_action

_TOKEN_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*")


def render_compact(result: GraphResult) -> str:
    aliases = _aliases(result.ranked_nodes)
    seed_line = _render_seed_aliases(result.seed_ids, aliases)
    alias_line = _render_aliases(result.ranked_nodes, aliases)
    node_lines = [
        _render_node(result.profile, node, aliases) for node in result.ranked_nodes
    ]
    edge_lines = [
        *_render_root_edges(result.seed_ids, result.ranked_nodes, aliases),
        *_render_edges(result.selected_edges, aliases),
    ]
    rank_line = ",".join(aliases[node.id] for node in result.ranked_nodes)
    frontier_line = _render_frontier(result, aliases)
    score_line = _render_scores(result, aliases)
    lines = [
        _render_header(result, seed_line),
        "legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next",
        alias_line,
        *node_lines,
        *edge_lines,
        f"rank={rank_line}",
        f"frontier={frontier_line}",
        *_render_profile_projection_lines(result, aliases),
        *_render_debug_projection_lines(result, aliases, score_line),
        f"profiles={','.join(result.profiles)}",
        f"omit={','.join(result.omit)}",
        f"avoid={','.join(result.avoid)}",
    ]
    return "\n".join(lines) + "\n"


def _render_header(result: GraphResult, seed_line: str) -> str:
    if result.profile.name == "failure-frontier":
        return (
            f"[search-failure] kind={_failure_kind(result)} profile={result.profile.name} "
            f"alg={ALGORITHM_ID} seed={seed_line} budget={result.budget}"
        )
    surface = "[graph-frontier]"
    return (
        f"{surface} profile={result.profile.name} alg={ALGORITHM_ID} "
        f"seed={seed_line} budget={result.budget}"
    )


def _failure_kind(result: GraphResult) -> str:
    for node in result.ranked_nodes:
        if node.kind == "failure":
            value = node.fields.get("failureKind") or node.role
            return str(value)
    return "failure"


def _render_debug_projection_lines(
    result: GraphResult, aliases: Mapping[str, str], score_line: str
) -> list[str]:
    if result.profile.name == "failure-frontier":
        return []
    return [
        f"scores={score_line}",
        f"paths={_render_paths(result, aliases)}",
        f"cache={_render_cache(result)}",
        f"trace={_render_trace(result)}",
        f"explain={_render_explanations(result, aliases)}",
        f"metrics={_render_metrics(result)}",
    ]


def _aliases(nodes: Iterable[Node]) -> Mapping[str, str]:
    prefixes = {
        "assert": "A",
        "collection": "C",
        "dependency": "D",
        "evidence": "E",
        "field": "F",
        "failure": "F",
        "finding": "F",
        "hot": "H",
        "item": "I",
        "key": "K",
        "owner": "O",
        "query": "Q",
        "range": "R",
        "symbol": "S",
        "test": "T",
        "type": "Y",
        "window": "W",
    }
    counts: dict[str, int] = {}
    aliases: dict[str, str] = {}
    for node in nodes:
        prefix = prefixes.get(node.kind, "N")
        counts[prefix] = counts.get(prefix, 0) + 1
        aliases[node.id] = (
            prefix if counts[prefix] == 1 else f"{prefix}{counts[prefix]}"
        )
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


def _render_profile_projection_lines(
    result: GraphResult, aliases: Mapping[str, str]
) -> list[str]:
    if result.profile.name == "owner-query":
        return _render_owner_query_projection_lines(result, aliases)
    if result.profile.name != "failure-frontier":
        return []
    lines = []
    actions = _render_frontier_actions(result, aliases)
    if actions:
        lines.append(f"frontierActions={actions}")
    lines.extend(
        [
            "queryProfiles=failure-frontier(F=>failure-facts+owners+hot-blocks),owner-query(O,K=>items+tests+dependency-usage),owner-tests(O=>covering-tests)",
            "entries=failure-frontier(F=>failure-facts+candidate-owners+hot-blocks+query-profiles)",
        ]
    )
    return lines


def _render_owner_query_projection_lines(
    result: GraphResult, aliases: Mapping[str, str]
) -> list[str]:
    actions = _render_owner_query_frontier_actions(result, aliases)
    if not actions:
        return []
    return [
        "pipeChoice=bounded-fanout maxBranches=3 repeat=false owner=asp-graph-turbo",
        "pipePolicy=maxSearchPipe=1 rewrite=false branchRepeat=false stopAfterProjectedBranches=true missingTokenSearch=false postProjectionSearch=false",
        "selectorPolicy=run-first reason=exact-selector-present before=search-reasoning",
        _render_query_token_coverage(result),
        f"frontierActions={actions}",
    ]


def _render_owner_query_frontier_actions(
    result: GraphResult, aliases: Mapping[str, str]
) -> str:
    branches = _owner_query_branches(result)
    actions: list[str] = []
    for index, node in enumerate(branches, start=1):
        selector = _selector_for_node(node)
        if selector is None:
            continue
        owner = node.fields.get("ownerPath") or node.fields.get("path")
        if owner is None:
            continue
        symbol = node.fields.get("symbol") or node.value
        alias = aliases.get(node.id, f"B{index}")
        actions.append(
            f"S{index}.selector(selector={selector},owner={owner},symbol={symbol},source={alias})!query-selector"
        )
        actions.append(
            f"R{index}.reasoning(owner={owner},source={alias})!search-reasoning"
        )
    return ",".join(actions)


def _render_query_token_coverage(result: GraphResult) -> str:
    query_tokens = _query_tokens(result)
    candidate_text = " ".join(
        _coverage_node_text(node)
        for node in result.ranked_nodes
        if node.kind != "query"
    ).lower()
    matched = [token for token in query_tokens if token in candidate_text]
    missing = [token for token in query_tokens if token not in candidate_text]
    return (
        "queryCoverage="
        f"matched={_comma_or_dash(matched)} "
        f"missing={_comma_or_dash(missing)} "
        "source=ranked-frontier"
    )


def _query_tokens(result: GraphResult) -> tuple[str, ...]:
    seen: set[str] = set()
    tokens: list[str] = []
    for node in result.ranked_nodes:
        if node.kind != "query" or node.id not in result.seed_ids:
            continue
        for token in _TOKEN_RE.findall(str(node.value).lower()):
            if token in seen:
                continue
            seen.add(token)
            tokens.append(token)
    return tuple(tokens)


def _coverage_node_text(node: Node) -> str:
    return " ".join(
        [
            str(node.value),
            str(node.kind),
            str(node.role),
            str(node.fields.get("symbol") or ""),
            str(node.fields.get("name") or ""),
            str(node.fields.get("path") or ""),
            str(node.fields.get("ownerPath") or ""),
            str(node.fields.get("matchText") or ""),
            _node_fields_text(node),
            _semantic_alias_text(node),
        ]
    )


def _node_fields_text(node: Node) -> str:
    fields = node.fields.get("fields")
    if not isinstance(fields, Mapping):
        return ""
    return " ".join(str(value) for value in fields.values())


def _semantic_alias_text(node: Node) -> str:
    aliases = {
        "collection": "collection collections list lists map maps set sets",
        "field": "field fields",
        "type": "type types",
    }
    return aliases.get(node.kind, "")


def _comma_or_dash(values: Iterable[str]) -> str:
    values = tuple(values)
    return ",".join(values) if values else "-"


def _owner_query_branches(result: GraphResult) -> list[Node]:
    hot_by_key = _owner_query_hot_nodes(result)
    candidates = [
        entry.node
        for entry in _prompt_frontier_entries(result)
        if entry.node.kind in {"field", "hot", "item"}
        and _selector_for_node(entry.node)
    ]
    field_candidates = [node for node in candidates if node.kind == "field"]
    if field_candidates:
        return _dedupe_mapped_branches(
            _owner_query_diverse_candidates(field_candidates),
            hot_by_key,
        )
    return _dedupe_mapped_branches(
        _owner_query_diverse_candidates(candidates),
        hot_by_key,
    )


def _dedupe_mapped_branches(
    branches: list[Node], hot_by_key: Mapping[tuple[str, str], Node]
) -> list[Node]:
    selected: list[Node] = []
    seen_selectors: set[str] = set()
    for node in branches:
        mapped = _hot_node_for_branch(node, hot_by_key)
        selector = _selector_for_node(mapped)
        if selector is None or selector in seen_selectors:
            continue
        seen_selectors.add(selector)
        selected.append(mapped)
    return selected


def _owner_query_diverse_candidates(candidates: list[Node]) -> list[Node]:
    preferred = [node for node in candidates if _source_preferred_node(node)]
    diverse: list[Node] = []
    fallback: list[Node] = []
    seen_selectors: set[str] = set()
    seen_symbols: set[str] = set()
    _append_symbol_diverse_branches(
        preferred, diverse, fallback, seen_selectors, seen_symbols
    )
    if len(diverse) < 3:
        _append_symbol_diverse_branches(
            candidates, diverse, fallback, seen_selectors, seen_symbols
        )
    for node in fallback:
        if len(diverse) >= 3:
            break
        selector = _selector_for_node(node)
        if selector is None or selector in seen_selectors:
            continue
        seen_selectors.add(selector)
        diverse.append(node)
    return diverse


def _owner_query_hot_nodes(result: GraphResult) -> dict[tuple[str, str], Node]:
    hot_nodes: dict[tuple[str, str], Node] = {}
    for entry in _prompt_frontier_entries(result):
        node = entry.node
        if node.kind != "hot" or _selector_for_node(node) is None:
            continue
        hot_nodes.setdefault(_branch_key(node), node)
    return hot_nodes


def _hot_node_for_branch(
    node: Node, hot_by_key: Mapping[tuple[str, str], Node]
) -> Node:
    if node.kind == "hot":
        return node
    return hot_by_key.get(_branch_key(node), node)


def _branch_key(node: Node) -> tuple[str, str]:
    owner = str(node.fields.get("ownerPath") or node.fields.get("path") or "")
    return owner, _branch_symbol(node)


def _append_symbol_diverse_branches(
    nodes: list[Node],
    diverse: list[Node],
    fallback: list[Node],
    seen_selectors: set[str],
    seen_symbols: set[str],
) -> None:
    for node in nodes:
        if len(diverse) >= 3:
            return
        selector = _selector_for_node(node)
        if selector is None or selector in seen_selectors:
            continue
        symbol = _branch_symbol(node)
        if symbol in seen_symbols:
            fallback.append(node)
            continue
        seen_selectors.add(selector)
        seen_symbols.add(symbol)
        diverse.append(node)


def _branch_symbol(node: Node) -> str:
    fields = node.fields.get("fields")
    field_name = fields.get("fieldName") if isinstance(fields, Mapping) else None
    return str(node.fields.get("symbol") or field_name or node.value).lower()


def _source_preferred_node(node: Node) -> bool:
    path = str(node.fields.get("path") or node.fields.get("ownerPath") or "")
    return not (
        "/tests/" in path
        or path.endswith("/tests")
        or "/benches/" in path
        or path.endswith("/benches")
        or "/examples/" in path
        or path.endswith("/examples")
        or "stress-test/" in path
    )


def _render_frontier_actions(result: GraphResult, aliases: Mapping[str, str]) -> str:
    actions = []
    for entry in result.frontier:
        node = entry.node
        if node.kind != "hot" or entry.action != "code":
            continue
        selector = _selector_for_node(node)
        if selector is None:
            continue
        language = node.fields.get("languageId") or "rust"
        actions.append(
            f"{aliases[node.id]}.code=>asp {language} query --selector {selector} --code ."
        )
    return ",".join(actions)


def _render_frontier(result: GraphResult, aliases: Mapping[str, str]) -> str:
    entries = _prompt_frontier_entries(result)
    return ",".join(f"{aliases[entry.node.id]}.{entry.action}" for entry in entries)


def _prompt_frontier_entries(result: GraphResult):
    if result.profile.name != "failure-frontier":
        return result.frontier
    rank_index = {node.id: index for index, node in enumerate(result.ranked_nodes)}
    entries = [
        entry
        for entry in result.frontier
        if entry.node.kind in {"assert", "hot", "key", "evidence"}
    ]
    entries.sort(
        key=lambda entry: (
            {"assert": 0, "hot": 1, "key": 2, "evidence": 3}.get(entry.node.kind, 9),
            rank_index.get(entry.node.id, 999),
        )
    )
    return tuple(entries) if entries else result.frontier


def _selector_for_node(node: Node) -> str | None:
    if node.kind == "field":
        fields = node.fields.get("fields")
        if isinstance(fields, Mapping):
            context_locator = fields.get("contextLocator")
            if context_locator is not None:
                return str(context_locator)
    locator = node.fields.get("locator") or node.fields.get("location")
    if locator is not None:
        return str(locator)
    path = node.fields.get("path")
    start = node.fields.get("startLine") or node.fields.get("start")
    end = node.fields.get("endLine") or node.fields.get("end")
    if path is not None and start is not None and end is not None:
        return f"{path}:{start}:{end}"
    return None


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
        f"{source}>{{{','.join(targets)}}}"
        for source, targets in sorted(groups.items())
    ]


def _render_scores(result: GraphResult, aliases: Mapping[str, str]) -> str:
    ranked_scores = [
        result.scores[node.id]
        for node in result.ranked_nodes
        if node.id in result.scores
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


def _render_paths(result: GraphResult, aliases: Mapping[str, str]) -> str:
    if not result.typed_paths:
        return "-"
    return ",".join(
        (
            f"{path.id}:{'>'.join(aliases.get(node_id, node_id) for node_id in path.node_ids)}"
            f":rel={'|'.join(path.relations) or '-'}"
            f":score={path.score:.2f}:cost={path.cost:.2f}:rank={path.rank}"
        )
        for path in result.typed_paths
    )


def _render_cache(result: GraphResult) -> str:
    cache = result.graph_cache
    key = cache.key
    if key.startswith("sha256:"):
        key = f"sha256:{key.removeprefix('sha256:')[:12]}"
    return f"{cache.status}:backend={cache.backend}:entries={cache.entries}:key={key}"


def _render_trace(result: GraphResult) -> str:
    return ",".join(
        f"{step.step}:{step.engine}{_render_trace_fields(step.fields)}"
        for step in result.algorithm_trace
    )


def _render_trace_fields(fields: Mapping[str, int | float | str | bool]) -> str:
    if not fields:
        return ""
    return (
        "(" + ",".join(f"{key}={value}" for key, value in sorted(fields.items())) + ")"
    )


def _render_explanations(result: GraphResult, aliases: Mapping[str, str]) -> str:
    return ";".join(
        f"{aliases.get(explanation.node_id, explanation.node_id)}:{'+'.join(explanation.reasons)}"
        for explanation in result.rank_explanations
    )


def _render_metrics(result: GraphResult) -> str:
    metrics = result.algorithm_metrics
    rendered = (
        f"nodes={metrics.node_count},edges={metrics.edge_count},"
        f"selectedEdges={metrics.selected_edge_count},reachable={metrics.reachable_node_count},"
        f"ranked={metrics.ranked_node_count},paths={metrics.path_count},"
        f"windows={metrics.merged_window_count},cache={metrics.cache_status}"
    )
    if (
        metrics.read_loop_direct_code_action_count
        or metrics.read_loop_duplicate_selector_count
        or metrics.read_loop_adjacent_range_window_count
        or metrics.read_loop_same_owner_scan_count
    ):
        rendered += (
            ",readLoop="
            f"code:{metrics.read_loop_direct_code_action_count}|"
            f"duplicate:{metrics.read_loop_duplicate_selector_count}|"
            f"adjacent:{metrics.read_loop_adjacent_range_window_count}|"
            f"sameOwner:{metrics.read_loop_same_owner_scan_count}"
        )
    if metrics.read_memory_suppressed_count:
        rendered += f",readMemorySuppressed={metrics.read_memory_suppressed_count}"
    return rendered
