"""Schema-owned graph turbo result packet projection."""

from __future__ import annotations

from .constants import ALGORITHM_ID
from .model import GraphProfile, GraphResult, MergedWindow, Node, ProfileCompatibility
from .profiles import frontier_action


def result_to_packet(result: GraphResult) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-result",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-result",
        "profile": result.profile.name,
        "algorithm": ALGORITHM_ID,
        "seedIds": list(result.seed_ids),
        "budget": result.budget,
        "kindBudgets": dict(result.kind_budgets),
        "profiles": list(result.profiles),
        "rank": [node.id for node in result.ranked_nodes],
        "rankedNodes": [_node_to_packet(result.profile, node) for node in result.ranked_nodes],
        "frontier": [
            {"nodeId": entry.node.id, "action": entry.action, "score": entry.score}
            for entry in result.frontier
        ],
        "scores": dict(result.scores),
        "edges": [
            {
                "source": edge.source,
                "target": edge.target,
                "relation": edge.relation,
                "weight": edge.weight,
            }
            for edge in result.selected_edges
        ],
        "mergedWindows": [_merged_window_to_packet(window) for window in result.merged_windows],
        "profileCompatibility": [
            _profile_compatibility_to_packet(entry) for entry in result.profile_compatibility
        ],
        "sourceSinkFrontier": {
            "sourceIds": list(result.source_sink_frontier.source_ids),
            "sinkIds": list(result.source_sink_frontier.sink_ids),
        },
        "typedPaths": [
            {
                "id": path.id,
                "pathKind": path.path_kind,
                "source": path.source,
                "sink": path.sink,
                "nodeIds": list(path.node_ids),
                "relations": list(path.relations),
                "cost": path.cost,
                "score": path.score,
                "rank": path.rank,
            }
            for path in result.typed_paths
        ],
        "flowLite": {"rankedPathIds": list(result.flow_lite.ranked_path_ids)},
        "packetFingerprint": result.packet_fingerprint,
        "graphCache": {
            "key": result.graph_cache.key,
            "status": result.graph_cache.status,
            "backend": result.graph_cache.backend,
            "entries": result.graph_cache.entries,
        },
        "algorithmTrace": [
            {"step": step.step, "engine": step.engine, "fields": dict(step.fields)}
            for step in result.algorithm_trace
        ],
        "rankExplanations": [
            {
                "nodeId": explanation.node_id,
                "score": explanation.score,
                "depth": explanation.depth,
                "reasons": list(explanation.reasons),
            }
            for explanation in result.rank_explanations
        ],
        "algorithmMetrics": {
            "nodeCount": result.algorithm_metrics.node_count,
            "edgeCount": result.algorithm_metrics.edge_count,
            "selectedEdgeCount": result.algorithm_metrics.selected_edge_count,
            "reachableNodeCount": result.algorithm_metrics.reachable_node_count,
            "rankedNodeCount": result.algorithm_metrics.ranked_node_count,
            "pathCount": result.algorithm_metrics.path_count,
            "mergedWindowCount": result.algorithm_metrics.merged_window_count,
            "cacheStatus": result.algorithm_metrics.cache_status,
        },
        "omit": list(result.omit),
        "avoid": list(result.avoid),
    }


def result_to_json(result: GraphResult) -> dict[str, object]:
    return result_to_packet(result)


def _node_to_packet(profile: GraphProfile, node: Node) -> dict[str, object]:
    return {
        "id": node.id,
        "kind": node.kind,
        "role": node.role,
        "value": node.value,
        "action": frontier_action(profile, node),
    }


def _merged_window_to_packet(window: MergedWindow) -> dict[str, object]:
    return {
        "path": window.path,
        "startLine": window.start_line,
        "endLine": window.end_line,
        "nodeIds": list(window.node_ids),
    }


def _profile_compatibility_to_packet(entry: ProfileCompatibility) -> dict[str, object]:
    return {
        "profile": entry.profile,
        "compatible": entry.compatible,
        "allowedRelations": list(entry.allowed_relations),
        "allowedTransitions": [
            {
                "sourceKind": transition.source_kind,
                "targetKind": transition.target_kind,
            }
            for transition in entry.allowed_transitions
        ],
        "kindBonus": dict(entry.kind_bonus),
        "frontierActions": dict(entry.frontier_actions),
    }
