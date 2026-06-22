"""Extract graph-turbo request evidence from recorded agent-session events."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .agent_session_graph_turbo_topology import (
    topology_membership_candidates_from_event,
)
from .utils import dict_value, list_value, optional_int, require_str


def graph_turbo_seed_plan_candidates_from_events(
    receipt: dict[str, Any],
    events: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    artifact_root = receipt.get("artifactRoot")
    root = (
        Path(artifact_root)
        if isinstance(artifact_root, str) and artifact_root
        else None
    )
    candidates = []
    seen: set[str] = set()
    for event in events:
        if event.get("kind") != "command.result":
            continue
        stdout_texts = _event_stdout_texts(event, root)
        for packet in _graph_turbo_request_packets(stdout_texts):
            seed_plan = dict_value(packet.get("seedPlan"))
            candidate = _candidate_from_seed_plan(event, packet, seed_plan)
            if not candidate or candidate["id"] in seen:
                continue
            candidates.append(candidate)
            seen.add(candidate["id"])
        for candidate in topology_membership_candidates_from_event(event, stdout_texts):
            if not candidate or candidate["id"] in seen:
                continue
            candidates.append(candidate)
            seen.add(candidate["id"])
    return candidates


def graph_turbo_request_packet_from_events(
    receipt: dict[str, Any],
    events: list[dict[str, Any]],
) -> dict[str, Any] | None:
    artifact_root = receipt.get("artifactRoot")
    root = (
        Path(artifact_root)
        if isinstance(artifact_root, str) and artifact_root
        else None
    )
    packets = [
        packet
        for event in events
        if event.get("kind") == "command.result"
        for packet in _graph_turbo_request_packets(_event_stdout_texts(event, root))
    ]
    for packet in packets:
        graph = dict_value(packet.get("graph"))
        if list_value(graph.get("nodes")):
            return packet
    return packets[0] if packets else None


def _graph_turbo_request_packets(
    stdout_texts: list[str],
) -> list[dict[str, Any]]:
    packets = []
    for text in stdout_texts:
        packet = _json_object(text)
        if packet.get("schemaId") == (
            "agent.semantic-protocols.semantic-graph-turbo-request"
        ):
            packets.append(packet)
    return packets


def _event_stdout_texts(
    event: dict[str, Any],
    artifact_root: Path | None,
) -> list[str]:
    texts = []
    if isinstance(event.get("preview"), str):
        texts.append(str(event["preview"]))
    if artifact_root is None:
        return texts
    for ref in list_value(event.get("artifactRefs")):
        if not isinstance(ref, dict) or ref.get("kind") != "stdout":
            continue
        path = ref.get("path")
        if not isinstance(path, str) or not path:
            continue
        output_path = artifact_root / path
        if output_path.is_file():
            texts.append(output_path.read_text(encoding="utf-8"))
    return texts


def _json_object(text: str) -> dict[str, Any]:
    try:
        value = json.loads(text.strip())
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def _candidate_from_seed_plan(
    event: dict[str, Any],
    packet: dict[str, Any],
    seed_plan: dict[str, Any],
) -> dict[str, Any] | None:
    if not seed_plan:
        return None
    reason = require_str(seed_plan, "reason", "unknown")
    selected = optional_int(seed_plan.get("selectedSeedCount")) or 0
    fallback = optional_int(seed_plan.get("fallbackOwnerSeedCount")) or 0
    query_present = bool(seed_plan.get("queryPresent"))
    query_seed_present = bool(seed_plan.get("querySeedPresent"))
    risk = [str(item) for item in list_value(seed_plan.get("riskFactors"))]
    if not risk:
        risk = _seed_plan_risk(packet, seed_plan)
    seed_quality = require_str(seed_plan, "seedQuality", "unknown")
    if query_seed_present and selected > 0 and not risk:
        return None
    command_id = require_str(
        event, "commandId", require_str(event, "eventId", "command")
    )
    packet_seed_ids = [str(item) for item in list_value(seed_plan.get("seedIds"))]
    owner_count = optional_int(seed_plan.get("candidateOwnerCount")) or 0
    candidate_count = optional_int(seed_plan.get("candidateCount")) or 0
    query_owner_seed_count = optional_int(seed_plan.get("queryOwnerSeedCount")) or 0
    recommended_actions = [
        str(item) for item in list_value(seed_plan.get("recommendedActions"))
    ]
    if selected == 0:
        expected_change = "non-empty-seed-frontier"
        recommended_action = (
            "Inspect graph-turbo seed extraction before rank calibration; no seed "
            "ids were selected."
        )
        confidence = 0.9
    elif "query-seed-missing" in risk:
        expected_change = "query-seed-present"
        recommended_action = (
            "Propagate the agent query into graph-turbo seedPlan before using "
            "owner fallback as calibration evidence."
        )
        confidence = 0.8
    elif "flat-query" in risk:
        expected_change = "split-query-pack"
        recommended_action = (
            "Split flat seed queries into cohesive query-pack clauses before "
            "graph-turbo rank calibration."
        )
        confidence = 0.8
    elif "owner-drift" in risk:
        expected_change = "narrow-owner-scope"
        recommended_action = (
            "Narrow owner scope before graph-turbo ranking when seed candidates "
            "span too many owners."
        )
        confidence = 0.75
    else:
        expected_change = "query-seed-present"
        recommended_action = (
            "Propagate the agent query into graph-turbo seedPlan before using "
            "owner fallback as calibration evidence."
        )
        confidence = 0.65 if fallback else 0.75
    return {
        "id": f"gt.seed-plan.{command_id}",
        "kind": "seed-plan-quality",
        "confidence": confidence,
        "reason": (
            "Graph-turbo seed phase selected "
            f"{selected} seed(s) with reason={reason}, "
            f"seedQuality={seed_quality}, "
            f"queryPresent={query_present}, querySeedPresent={query_seed_present}, "
            f"queryOwnerSeedCount={query_owner_seed_count}, "
            f"fallbackOwnerSeedCount={fallback}, "
            f"candidateOwnerCount={owner_count}, candidateCount={candidate_count}, "
            f"risk={','.join(risk) or 'none'}, "
            f"recommendedActions={','.join(recommended_actions) or 'none'}."
        ),
        "evidenceRefs": [require_str(event, "eventId", command_id)],
        "packetNodeIds": packet_seed_ids,
        "seedQuality": seed_quality,
        "riskFactors": risk,
        "recommendedActions": recommended_actions,
        "expectedChange": expected_change,
        "recommendedAction": recommended_action,
    }


def _seed_plan_risk(
    packet: dict[str, Any],
    seed_plan: dict[str, Any],
) -> list[str]:
    query_term_count = len(list_value(packet.get("queryTerms")))
    owner_count = optional_int(seed_plan.get("candidateOwnerCount")) or 0
    fallback = optional_int(seed_plan.get("fallbackOwnerSeedCount")) or 0
    selected = optional_int(seed_plan.get("selectedSeedCount")) or 0
    risk = []
    if selected == 0:
        risk.append("empty-seed-frontier")
    if fallback:
        risk.append("fallback-owner")
    if query_term_count >= 6:
        risk.append("flat-query")
    if query_term_count >= 4 and owner_count >= 4:
        risk.append("owner-drift")
    return risk
