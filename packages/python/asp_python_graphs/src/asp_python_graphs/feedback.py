"""Receipt-driven graph-turbo feedback packet helpers."""

from __future__ import annotations

import copy
import hashlib
import re
from collections.abc import Iterable, Mapping
from typing import Any


_FEEDBACK_SCHEMA_ID = "agent.semantic-protocols.semantic-graph-turbo-feedback"
_FEEDBACK_PROTOCOL_ID = "agent.semantic-protocols.semantic-fact-frontier-feedback"

_SELECTOR_RE = re.compile(r"--selector(?:=|\s+)(?P<selector>[^\s]+)")
_SUCCESS_DELTA = 0.85
_WASTE_DELTA = -0.75


def feedback_packet_from_sandtable(
    report: Mapping[str, Any],
    *,
    source_path: str | None = None,
) -> dict[str, object]:
    """Build schema-shaped feedback facts from a sandtable JSON report."""

    nodes = [
        node
        for scenario_index, scenario in enumerate(_scenarios(report))
        for node in _feedback_nodes_for_scenario(scenario, scenario_index)
    ]
    success_count = sum(1 for node in nodes if _node_effect(node) == "boost")
    penalty_count = sum(1 for node in nodes if _node_effect(node) == "penalty")

    return {
        "schemaId": _FEEDBACK_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": _FEEDBACK_PROTOCOL_ID,
        "protocolVersion": "1",
        "packetKind": "graph-turbo-feedback",
        "source": "sandtable",
        "sourcePath": source_path,
        "graph": {"nodes": nodes, "edges": []},
        "metrics": {
            "receiptNodeCount": len(nodes),
            "receiptEdgeCount": 0,
            "successCount": success_count,
            "penaltyCount": penalty_count,
        },
    }


def merge_feedback_into_packet(
    packet: Mapping[str, Any],
    feedback_packets: Iterable[Mapping[str, Any]],
) -> dict[str, Any]:
    """Return a graph-turbo request packet with feedback graph facts appended."""

    merged = copy.deepcopy(dict(packet))
    graph = merged.get("graph")
    if not isinstance(graph, dict):
        graph = merged
    nodes = graph.setdefault("nodes", [])
    edges = graph.setdefault("edges", [])
    if not isinstance(nodes, list) or not isinstance(edges, list):
        raise SystemExit("graph turbo packet graph.nodes and graph.edges must be lists")
    node_ids = _node_ids(nodes)
    for feedback in feedback_packets:
        _append_feedback_graph(feedback, nodes, edges, node_ids)
    return merged


def _feedback_nodes_for_scenario(
    scenario: Mapping[str, Any],
    scenario_index: int,
) -> tuple[dict[str, object], ...]:
    scenario_id = _entry_id(scenario, "scenario", scenario_index)
    return tuple(
        node
        for step_index, step in enumerate(_steps(scenario))
        for node in _feedback_nodes_for_step(scenario_id, step, step_index)
    )


def _feedback_nodes_for_step(
    scenario_id: str,
    step: Mapping[str, Any],
    step_index: int,
) -> tuple[dict[str, object], ...]:
    selectors = _selectors_from_step(step)
    if not selectors:
        return ()
    step_id = _entry_id(step, "step", step_index)
    return (
        *_success_nodes(scenario_id, step_id, step, selectors),
        *_duplicate_nodes(scenario_id, step_id, selectors),
    )


def _success_nodes(
    scenario_id: str,
    step_id: str,
    step: Mapping[str, Any],
    selectors: tuple[str, ...],
) -> tuple[dict[str, object], ...]:
    if not _step_has_final_answer(step):
        return ()
    return (
        _receipt_node(
            selector=selectors[0],
            effect="boost",
            reason="frontier-success",
            score_delta=_SUCCESS_DELTA,
            scenario_id=scenario_id,
            step_id=step_id,
        ),
        *(
            _receipt_node(
                selector=selector,
                effect="penalty",
                reason="extra-selector-after-primary",
                score_delta=_WASTE_DELTA,
                scenario_id=scenario_id,
                step_id=step_id,
            )
            for selector in selectors[1:]
        ),
    )


def _duplicate_nodes(
    scenario_id: str,
    step_id: str,
    selectors: tuple[str, ...],
) -> tuple[dict[str, object], ...]:
    return tuple(
        _receipt_node(
            selector=selector,
            effect="penalty",
            reason="duplicate-selector",
            score_delta=_WASTE_DELTA,
            scenario_id=scenario_id,
            step_id=step_id,
        )
        for selector in _duplicate_selectors(selectors)
    )


def _append_feedback_graph(
    feedback: Mapping[str, Any],
    nodes: list[object],
    edges: list[object],
    node_ids: set[str],
) -> None:
    feedback_graph = feedback.get("graph")
    if not isinstance(feedback_graph, Mapping):
        raise SystemExit("graph turbo feedback packet must contain graph object")
    for node in _mapping_list(feedback_graph, "nodes"):
        _append_feedback_node(nodes, node_ids, node)
    for edge in _mapping_list(feedback_graph, "edges"):
        _append_feedback_edge(edges, node_ids, edge)


def _append_feedback_node(
    nodes: list[object],
    node_ids: set[str],
    node: Mapping[str, Any],
) -> None:
    node_id = node.get("id")
    if not isinstance(node_id, str) or not node_id or node_id in node_ids:
        return
    nodes.append(dict(node))
    node_ids.add(node_id)


def _append_feedback_edge(
    edges: list[object],
    node_ids: set[str],
    edge: Mapping[str, Any],
) -> None:
    source = edge.get("source")
    target = edge.get("target")
    if not isinstance(source, str) or not isinstance(target, str):
        return
    if source not in node_ids or target not in node_ids:
        return
    edges.append(dict(edge))


def _node_ids(nodes: Iterable[object]) -> set[str]:
    return {
        node["id"]
        for node in nodes
        if isinstance(node, Mapping) and isinstance(node.get("id"), str)
    }


def _node_effect(node: Mapping[str, object]) -> str | None:
    fields = node.get("fields")
    if isinstance(fields, Mapping):
        effect = fields.get("effect")
        if isinstance(effect, str):
            return effect
    return None


def _scenarios(report: Mapping[str, Any]) -> tuple[Mapping[str, Any], ...]:
    scenarios = report.get("scenarios")
    if isinstance(scenarios, list):
        return tuple(item for item in scenarios if isinstance(item, Mapping))
    return (report,)


def _steps(scenario: Mapping[str, Any]) -> tuple[Mapping[str, Any], ...]:
    steps = scenario.get("steps")
    if isinstance(steps, list):
        return tuple(item for item in steps if isinstance(item, Mapping))
    return ()


def _entry_id(entry: Mapping[str, Any], fallback: str, index: int) -> str:
    value = entry.get("id") or entry.get(f"{fallback}Id")
    if isinstance(value, str) and value:
        return value
    return f"{fallback}-{index}"


def _selectors_from_step(step: Mapping[str, Any]) -> tuple[str, ...]:
    observations = step.get("observations")
    if not isinstance(observations, Mapping):
        return ()
    pipe_flow = observations.get("pipeFlow")
    if not isinstance(pipe_flow, Mapping):
        return ()
    commands = pipe_flow.get("commands")
    if not isinstance(commands, list):
        return ()
    selectors: list[str] = []
    seen: set[str] = set()
    for command in commands:
        if not isinstance(command, str):
            continue
        match = _SELECTOR_RE.search(command)
        if match is None:
            continue
        selector = match.group("selector")
        if selector in seen:
            selectors.append(selector)
            continue
        seen.add(selector)
        selectors.append(selector)
    return tuple(selectors)


def _duplicate_selectors(selectors: Iterable[str]) -> tuple[str, ...]:
    seen: set[str] = set()
    duplicates: list[str] = []
    for selector in selectors:
        if selector in seen:
            duplicates.append(selector)
            continue
        seen.add(selector)
    return tuple(duplicates)


def _step_has_final_answer(step: Mapping[str, Any]) -> bool:
    if step.get("status") != "pass":
        return False
    observations = step.get("observations")
    if not isinstance(observations, Mapping):
        return False
    answer = observations.get("finalAnswer")
    if not isinstance(answer, Mapping):
        return False
    return bool(answer.get("present")) and bool(answer.get("afterLastToolUse"))


def _receipt_node(
    *,
    selector: str,
    effect: str,
    reason: str,
    score_delta: float,
    scenario_id: str,
    step_id: str,
) -> dict[str, object]:
    owner = selector.split(":", 1)[0] if ":" in selector else selector
    node_id = "receipt:" + _stable_id(scenario_id, step_id, selector, reason)
    return {
        "id": node_id,
        "kind": "receipt",
        "role": "frontier-feedback",
        "value": f"{effect}:{reason}:{selector}",
        "fields": {
            "receiptKind": "frontier-success"
            if effect == "boost"
            else "frontier-waste",
            "effect": effect,
            "reason": reason,
            "selector": selector,
            "scope": "exact-selector",
            "ownerPath": owner,
            "scoreDelta": score_delta,
            "sourceScenario": scenario_id,
            "sourceStep": step_id,
        },
    }


def _stable_id(*parts: str) -> str:
    digest = hashlib.sha256("\0".join(parts).encode("utf-8")).hexdigest()
    return digest[:16]


def _mapping_list(
    source: Mapping[str, Any], name: str
) -> tuple[Mapping[str, Any], ...]:
    value = source.get(name, [])
    if not isinstance(value, list) or not all(
        isinstance(item, Mapping) for item in value
    ):
        raise SystemExit(f"graph turbo feedback graph.{name} must be an object array")
    return tuple(value)
