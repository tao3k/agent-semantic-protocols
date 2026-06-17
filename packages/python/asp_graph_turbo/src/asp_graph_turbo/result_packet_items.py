"""Small packet projections for graph turbo result items."""

from __future__ import annotations

from .model import (
    GraphProfile,
    MergedWindow,
    Node,
    ProfileCompatibility,
    ProfileMatrixSummary,
    ReceiptAdjustment,
)
from .profiles import frontier_action


def node_to_packet(profile: GraphProfile, node: Node) -> dict[str, object]:
    return {
        "id": node.id,
        "kind": node.kind,
        "role": node.role,
        "value": node.value,
        "action": frontier_action(profile, node),
    }


def merged_window_to_packet(window: MergedWindow) -> dict[str, object]:
    return {
        "path": window.path,
        "startLine": window.start_line,
        "endLine": window.end_line,
        "nodeIds": list(window.node_ids),
    }


def profile_compatibility_to_packet(entry: ProfileCompatibility) -> dict[str, object]:
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


def profile_matrix_to_packet(entry: ProfileMatrixSummary) -> dict[str, object]:
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


def receipt_adjustment_to_packet(entry: ReceiptAdjustment) -> dict[str, object]:
    return {
        "nodeId": entry.node_id,
        "effect": entry.effect,
        "scoreDelta": entry.score_delta,
        "reason": entry.reason,
    }
