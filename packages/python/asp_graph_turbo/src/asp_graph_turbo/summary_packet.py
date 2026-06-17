"""Schema-owned compact summary packet projection for graph turbo results."""

from __future__ import annotations

from collections.abc import Mapping

from .constants import ALGORITHM_ID
from .model import GraphProfile, GraphResult, Node, ProfileCompatibility
from .packet import result_to_packet
from .profiles import frontier_action
from .selector import (
    graph_turbo_node_range,
    graph_turbo_owner_path_for_node,
    graph_turbo_selector_for_node,
)


def result_to_summary_packet(result: GraphResult) -> dict[str, object]:
    """Return a schema-shaped summary that preserves next-action facts."""

    full = result_to_packet(result)
    packet = {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-summary",
        "schemaVersion": "1",
        "protocolId": full["protocolId"],
        "protocolVersion": full["protocolVersion"],
        "packetKind": "graph-turbo-summary",
        "summaryKind": "frontier-projection",
        "sourceSchemaId": full["schemaId"],
        "sourcePacketKind": full["packetKind"],
        "profile": result.profile.name,
        "algorithm": ALGORITHM_ID,
        "seedIds": list(result.seed_ids),
        "budget": result.budget,
        "kindBudgets": dict(result.kind_budgets),
        "profiles": list(result.profiles),
        "rank": [node.id for node in result.ranked_nodes],
        "rankedNodes": [
            _node_summary(result.profile, node, result.scores)
            for node in result.ranked_nodes
        ],
        "frontier": [
            _frontier_summary(result.profile, entry.node, entry.action, entry.score)
            for entry in result.frontier
        ],
        "edges": full["edges"],
        "sourceSinkFrontier": full["sourceSinkFrontier"],
        "typedPaths": full["typedPaths"],
        "flowLite": full["flowLite"],
        "packetFingerprint": full["packetFingerprint"],
        "graphCache": full["graphCache"],
        "algorithmTrace": full["algorithmTrace"],
        "rankExplanations": full["rankExplanations"],
        "receiptAdjustments": full["receiptAdjustments"],
        "evidenceReliability": full["evidenceReliability"],
        "profileCompatibility": [
            _profile_compatibility_summary(entry)
            for entry in result.profile_compatibility
            if entry.profile == result.profile.name
        ],
        "profileMatrices": [
            entry
            for entry in full["profileMatrices"]
            if entry["profile"] == result.profile.name
        ],
        "algorithmMetrics": full["algorithmMetrics"],
        "omit": list(result.omit),
        "avoid": list(result.avoid),
        "projection": {
            "included": [
                "frontier-selectors",
                "ranked-node-locators",
                "selected-edges",
                "typed-paths",
                "profile-matrices",
                "algorithm-trace",
                "algorithm-metrics",
            ],
            "omitted": [
                "full-score-vector",
                "full-node-fields",
                "profile-transition-tables",
                "non-active-profile-matrices",
                "source-code",
            ],
        },
    }
    if "readMemory" in full:
        packet["readMemory"] = full["readMemory"]
    return packet


def _node_summary(
    profile: GraphProfile, node: Node, scores: Mapping[str, float]
) -> dict[str, object]:
    return {
        "id": node.id,
        "kind": node.kind,
        "role": node.role,
        "value": node.value,
        "action": frontier_action(profile, node) or node.action,
        "score": scores.get(node.id),
        "selector": graph_turbo_selector_for_node(node),
        "owner": graph_turbo_owner_path_for_node(node),
        "symbol": _node_symbol(node),
        "range": _node_range(node),
    }


def _frontier_summary(
    profile: GraphProfile, node: Node, action: str, score: float
) -> dict[str, object]:
    summary = _node_summary(profile, node, {node.id: score})
    summary["nodeId"] = summary.pop("id")
    summary["action"] = action
    summary["score"] = score
    return summary


def _node_symbol(node: Node) -> str | None:
    symbol = node.fields.get("symbol")
    if isinstance(symbol, str) and symbol:
        return symbol
    nested = node.fields.get("fields")
    if isinstance(nested, Mapping):
        for key in ("symbol", "fieldName", "typeName", "collectionName", "name"):
            value = nested.get(key)
            if isinstance(value, str) and value:
                return value
    return node.value if node.value else None


def _node_range(node: Node) -> dict[str, object] | None:
    node_range = graph_turbo_node_range(node)
    if node_range is None:
        return None
    return {
        "path": node_range.path,
        "startLine": node_range.start_line,
        "endLine": node_range.end_line,
    }


def _profile_compatibility_summary(
    entry: ProfileCompatibility,
) -> dict[str, object]:
    return {
        "profile": entry.profile,
        "compatible": entry.compatible,
        "allowedRelationCount": len(entry.allowed_relations),
        "allowedTransitionCount": len(entry.allowed_transitions),
    }
