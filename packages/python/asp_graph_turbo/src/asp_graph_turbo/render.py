"""Graph-frontier compact rendering for graph turbo results."""

from __future__ import annotations

import re
from collections.abc import Iterable, Mapping

from .constants import ALGORITHM_ID
from .evidence_reliability import evidence_reliability_report
from .frontier_actions import FrontierAction, frontier_action_items
from .model import Edge, GraphProfile, GraphResult, Node
from .profiles import frontier_action

_TOKEN_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*")
_EVIDENCE_QUALITY_PROFILES = {"evidence-quality", "rust-evidence-quality"}


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
        *_render_read_memory_lines(result),
        *_render_evidence_reliability_lines(result, aliases),
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
        "evidence-gap": "GAP",
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


def _render_read_memory_lines(result: GraphResult) -> list[str]:
    projection = result.read_memory
    if not projection.seen_selectors and not projection.suppressed_selectors:
        return []
    return [
        "readMemory="
        f"seen={_comma_or_dash(projection.seen_selectors)} "
        f"suppressed={_comma_or_dash(projection.suppressed_selectors)}"
    ]


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
    return _render_frontier_actions(result, aliases)


def _render_evidence_reliability_lines(
    result: GraphResult, aliases: Mapping[str, str]
) -> list[str]:
    if result.profile.name not in _EVIDENCE_QUALITY_PROFILES:
        return []
    report = evidence_reliability_report(result)
    gates = report["gates"]
    findings = report["findings"]
    lines = [
        "reliability="
        f"{'pass' if report['reliable'] else 'fail'} "
        f"score={report['score']} "
        f"blocking={report['blockingCount']} "
        f"findings={report['findingCount']} "
        f"gates={_comma_or_dash(gates)}"
    ]
    if findings:
        lines.append(f"reliabilityFindings={_render_reliability_findings(findings, aliases)}")
    return lines


def _render_reliability_findings(
    findings: object, aliases: Mapping[str, str], limit: int = 5
) -> str:
    if not isinstance(findings, list):
        return "-"
    rendered: list[str] = []
    for index, finding in enumerate(findings[:limit], start=1):
        if not isinstance(finding, Mapping):
            continue
        node_id = str(finding.get("nodeId") or "-")
        node = aliases.get(node_id, node_id)
        rendered.append(
            "R"
            f"{index}:{finding.get('severity')}:{finding.get('kind')}:"
            f"{node}!{finding.get('action')}"
        )
    return ",".join(rendered) if rendered else "-"


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


def _render_frontier_actions(result: GraphResult, aliases: Mapping[str, str]) -> str:
    return ",".join(
        _render_frontier_action(action, aliases)
        for action in frontier_action_items(result)
    )


def _render_frontier_action(
    action: FrontierAction, aliases: Mapping[str, str]
) -> str:
    source = aliases.get(action.source_node_id, action.source_node_id)
    if action.action_kind == "selector":
        return (
            f"{action.action_id}.selector(selector={action.selector},owner={action.owner},"
            f"symbol={action.symbol},source={source})!{action.next}"
        )
    if action.action_kind == "reasoning":
        return (
            f"{action.action_id}.reasoning(owner={action.owner},source={source})!"
            f"{action.next}"
        )
    if action.action_kind == "query-code":
        language = action.fields.get("languageId") or "-"
        return (
            f"{action.action_id}.query-code(selector={action.selector},owner={action.owner},"
            f"symbol={action.symbol},source={source},language={language})!{action.next}"
        )
    return (
        f"{action.action_id}.{action.action_kind}(target={action.target},source={source})!"
        f"{action.next}"
    )


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
        f"pathBackend={metrics.path_backend},pathPairs={metrics.path_pair_count},"
        f"pathCandidates={metrics.path_candidate_count},pathFallbacks={metrics.path_fallback_count},"
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
    if metrics.read_loop_second_pass_suppressed_count:
        rendered += (
            ",readLoopSecondPass="
            f"duplicate:{metrics.read_loop_duplicate_selector_suppressed_count}|"
            f"adjacentMerged:{metrics.read_loop_adjacent_range_merged_count}|"
            f"sameOwner:{metrics.read_loop_same_owner_suppressed_count}"
        )
    if metrics.relation_channel_count:
        rendered += f",relationChannels={metrics.relation_channel_count}"
    if metrics.ppr_iterations:
        rendered += (
            f",ppr=iter:{metrics.ppr_iterations}|"
            f"residual:{metrics.ppr_residual:.2e}|mass:{metrics.ppr_mass_sum:.2f}"
        )
    return rendered
