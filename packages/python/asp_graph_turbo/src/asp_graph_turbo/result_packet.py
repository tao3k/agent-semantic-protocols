"""Graph turbo result packet assembly."""

from __future__ import annotations

from .constants import ALGORITHM_ID
from .evidence_reliability import evidence_reliability_report
from .frontier_actions import frontier_action_packets
from .model import GraphResult
from .result_packet_items import (
    merged_window_to_packet,
    node_to_packet,
    profile_compatibility_to_packet,
    profile_matrix_to_packet,
    receipt_adjustment_to_packet,
)


def result_to_packet(result: GraphResult) -> dict[str, object]:
    packet = {
        **_identity_section(result),
        **_rank_section(result),
        **_frontier_section(result),
        **_graph_projection_section(result),
        **_profile_projection_section(result),
        **_trace_section(result),
        "algorithmMetrics": _algorithm_metrics_to_packet(result),
        "omit": list(result.omit),
        "avoid": list(result.avoid),
    }
    _append_read_memory(packet, result)
    return packet


def result_to_json(result: GraphResult) -> dict[str, object]:
    return result_to_packet(result)


def _identity_section(result: GraphResult) -> dict[str, object]:
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
    }


def _rank_section(result: GraphResult) -> dict[str, object]:
    return {
        "rank": [node.id for node in result.ranked_nodes],
        "rankedNodes": [
            node_to_packet(result.profile, node) for node in result.ranked_nodes
        ],
        "scores": dict(result.scores),
        "rankExplanations": [
            {
                "nodeId": explanation.node_id,
                "score": explanation.score,
                "depth": explanation.depth,
                "reasons": list(explanation.reasons),
            }
            for explanation in result.rank_explanations
        ],
    }


def _frontier_section(result: GraphResult) -> dict[str, object]:
    return {
        "frontier": [
            {"nodeId": entry.node.id, "action": entry.action, "score": entry.score}
            for entry in result.frontier
        ],
        "frontierActions": frontier_action_packets(result),
    }


def _graph_projection_section(result: GraphResult) -> dict[str, object]:
    return {
        "edges": [
            {
                "source": edge.source,
                "target": edge.target,
                "relation": edge.relation,
                "weight": edge.weight,
                "originalSource": edge.original_source,
                "originalTarget": edge.original_target,
                "reversed": edge.reversed,
            }
            for edge in result.selected_edges
        ],
        "mergedWindows": [
            merged_window_to_packet(window) for window in result.merged_windows
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
    }


def _profile_projection_section(result: GraphResult) -> dict[str, object]:
    return {
        "profileCompatibility": [
            profile_compatibility_to_packet(entry)
            for entry in result.profile_compatibility
        ],
        "profileMatrices": [
            profile_matrix_to_packet(entry) for entry in result.profile_matrices
        ],
    }


def _trace_section(result: GraphResult) -> dict[str, object]:
    return {
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
        "receiptAdjustments": [
            receipt_adjustment_to_packet(adjustment)
            for adjustment in result.receipt_adjustments
        ],
        "evidenceReliability": evidence_reliability_report(result),
    }


def _algorithm_metrics_to_packet(result: GraphResult) -> dict[str, object]:
    metrics = result.algorithm_metrics
    return {
        "nodeCount": metrics.node_count,
        "edgeCount": metrics.edge_count,
        "selectedEdgeCount": metrics.selected_edge_count,
        "reachableNodeCount": metrics.reachable_node_count,
        "rankedNodeCount": metrics.ranked_node_count,
        "pathCount": metrics.path_count,
        "pathBackend": metrics.path_backend,
        "pathFallbackCount": metrics.path_fallback_count,
        "pathPairCount": metrics.path_pair_count,
        "pathCandidateCount": metrics.path_candidate_count,
        "mergedWindowCount": metrics.merged_window_count,
        "cacheStatus": metrics.cache_status,
        "readLoopDirectCodeActionCount": metrics.read_loop_direct_code_action_count,
        "readLoopDuplicateSelectorCount": metrics.read_loop_duplicate_selector_count,
        "readLoopAdjacentRangeWindowCount": (
            metrics.read_loop_adjacent_range_window_count
        ),
        "readLoopSameOwnerScanCount": metrics.read_loop_same_owner_scan_count,
        "readMemorySuppressedCount": metrics.read_memory_suppressed_count,
        "receiptBoostCount": metrics.receipt_boost_count,
        "receiptPenaltyCount": metrics.receipt_penalty_count,
        "relationChannelCount": metrics.relation_channel_count,
        "pprIterations": metrics.ppr_iterations,
        "pprResidual": metrics.ppr_residual,
        "pprDanglingMassLast": metrics.ppr_dangling_mass_last,
        "pprMassSum": metrics.ppr_mass_sum,
        "readLoopSecondPassSuppressedCount": (
            metrics.read_loop_second_pass_suppressed_count
        ),
        "readLoopDuplicateSelectorSuppressedCount": (
            metrics.read_loop_duplicate_selector_suppressed_count
        ),
        "readLoopAdjacentRangeMergedCount": (
            metrics.read_loop_adjacent_range_merged_count
        ),
        "readLoopSameOwnerSuppressedCount": metrics.read_loop_same_owner_suppressed_count,
        "querySeedPriorCount": metrics.query_seed_prior_count,
        "querySeedPriorMass": metrics.query_seed_prior_mass,
        "queryPackageCohesionCount": metrics.query_package_cohesion_count,
        "queryPackageDriftPenaltyCount": metrics.query_package_drift_penalty_count,
        "queryPackageCohesionDelta": metrics.query_package_cohesion_delta,
        "queryClauseCoverageCount": metrics.query_clause_coverage_count,
        "queryClauseCoverageDelta": metrics.query_clause_coverage_delta,
    }


def _append_read_memory(packet: dict[str, object], result: GraphResult) -> None:
    projection = result.read_memory
    if not projection.seen_selectors and not projection.suppressed_selectors:
        return
    packet["readMemory"] = {
        "seenSelectors": list(projection.seen_selectors),
        "suppressedSelectors": list(projection.suppressed_selectors),
    }
