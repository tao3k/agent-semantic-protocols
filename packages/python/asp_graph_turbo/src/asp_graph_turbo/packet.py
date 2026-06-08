"""Schema-owned graph turbo result packet projection."""

from __future__ import annotations

from .constants import ALGORITHM_ID
from .model import (
    GraphProfile,
    GraphResult,
    MergedWindow,
    Node,
    ProfileCompatibility,
    ProfileMatrixSummary,
    ReceiptAdjustment,
)
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
        "rankedNodes": [
            _node_to_packet(result.profile, node) for node in result.ranked_nodes
        ],
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
                "originalSource": edge.original_source,
                "originalTarget": edge.original_target,
                "reversed": edge.reversed,
            }
            for edge in result.selected_edges
        ],
        "mergedWindows": [
            _merged_window_to_packet(window) for window in result.merged_windows
        ],
        "profileCompatibility": [
            _profile_compatibility_to_packet(entry)
            for entry in result.profile_compatibility
        ],
        "profileMatrices": [
            _profile_matrix_to_packet(entry) for entry in result.profile_matrices
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
        "receiptAdjustments": [
            _receipt_adjustment_to_packet(adjustment)
            for adjustment in result.receipt_adjustments
        ],
        "algorithmMetrics": {
            "nodeCount": result.algorithm_metrics.node_count,
            "edgeCount": result.algorithm_metrics.edge_count,
            "selectedEdgeCount": result.algorithm_metrics.selected_edge_count,
            "reachableNodeCount": result.algorithm_metrics.reachable_node_count,
            "rankedNodeCount": result.algorithm_metrics.ranked_node_count,
            "pathCount": result.algorithm_metrics.path_count,
            "pathBackend": result.algorithm_metrics.path_backend,
            "pathFallbackCount": result.algorithm_metrics.path_fallback_count,
            "pathPairCount": result.algorithm_metrics.path_pair_count,
            "pathCandidateCount": result.algorithm_metrics.path_candidate_count,
            "mergedWindowCount": result.algorithm_metrics.merged_window_count,
            "cacheStatus": result.algorithm_metrics.cache_status,
            "readLoopDirectCodeActionCount": (
                result.algorithm_metrics.read_loop_direct_code_action_count
            ),
            "readLoopDuplicateSelectorCount": (
                result.algorithm_metrics.read_loop_duplicate_selector_count
            ),
            "readLoopAdjacentRangeWindowCount": (
                result.algorithm_metrics.read_loop_adjacent_range_window_count
            ),
            "readLoopSameOwnerScanCount": (
                result.algorithm_metrics.read_loop_same_owner_scan_count
            ),
            "readMemorySuppressedCount": (
                result.algorithm_metrics.read_memory_suppressed_count
            ),
            "receiptBoostCount": result.algorithm_metrics.receipt_boost_count,
            "receiptPenaltyCount": result.algorithm_metrics.receipt_penalty_count,
            "relationChannelCount": result.algorithm_metrics.relation_channel_count,
            "pprIterations": result.algorithm_metrics.ppr_iterations,
            "pprResidual": result.algorithm_metrics.ppr_residual,
            "pprDanglingMassLast": (result.algorithm_metrics.ppr_dangling_mass_last),
            "pprMassSum": result.algorithm_metrics.ppr_mass_sum,
            "readLoopSecondPassSuppressedCount": (
                result.algorithm_metrics.read_loop_second_pass_suppressed_count
            ),
            "readLoopDuplicateSelectorSuppressedCount": (
                result.algorithm_metrics.read_loop_duplicate_selector_suppressed_count
            ),
            "readLoopAdjacentRangeMergedCount": (
                result.algorithm_metrics.read_loop_adjacent_range_merged_count
            ),
            "readLoopSameOwnerSuppressedCount": (
                result.algorithm_metrics.read_loop_same_owner_suppressed_count
            ),
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
        "relationWeightMultiplier": dict(entry.relation_weight_multiplier),
        "frontierActions": dict(entry.frontier_actions),
    }


def _profile_matrix_to_packet(entry: ProfileMatrixSummary) -> dict[str, object]:
    return {
        "profile": entry.profile,
        "relationCount": entry.relation_count,
        "transitionCount": entry.transition_count,
        "supportedEdgeCount": entry.supported_edge_count,
        "reachableEdgeCount": entry.reachable_edge_count,
        "density": entry.density,
        "relationMatrixCount": entry.relation_matrix_count,
        "zeroEdgeRelationCount": entry.zero_edge_relation_count,
        "transitionNonZeroCount": entry.transition_nonzero_count,
        "transitionWeightMass": entry.transition_weight_mass,
        "relationChannels": [
            {
                "relation": channel.relation,
                "supportedEdgeCount": channel.supported_edge_count,
                "reachableEdgeCount": channel.reachable_edge_count,
                "weightMass": channel.weight_mass,
                "reachableWeightMass": channel.reachable_weight_mass,
                "matrixNonZeroCount": channel.matrix_nonzero_count,
                "rankedContributionMass": channel.ranked_contribution_mass,
                "frontierContributionMass": channel.frontier_contribution_mass,
            }
            for channel in entry.relation_channels
        ],
    }


def _receipt_adjustment_to_packet(entry: ReceiptAdjustment) -> dict[str, object]:
    return {
        "nodeId": entry.node_id,
        "effect": entry.effect,
        "scoreDelta": entry.score_delta,
        "reason": entry.reason,
    }
