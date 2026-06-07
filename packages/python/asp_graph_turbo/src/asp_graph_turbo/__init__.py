"""Typed graph reasoning helpers for ASP client-side frontier ranking."""

from .model import (
    AllowedTransition,
    AlgorithmMetrics,
    AlgorithmTraceStep,
    Edge,
    FlowLite,
    FrontierEntry,
    GraphCache,
    GraphProfile,
    GraphResult,
    MergedWindow,
    Node,
    ProfileCompatibility,
    ProfileMatrixSummary,
    RankExplanation,
    ReceiptAdjustment,
    ReadLoopGuard,
    SourceSinkFrontier,
    TypedPath,
    TypedGraph,
)

_TURBO_EXPORTS = {
    "DEFAULT_PROFILES",
    "ontology_catalog_to_graph_packet",
    "ontology_catalog_to_graph_request",
    "rank_frontier",
    "render_compact",
    "result_to_packet",
}

__all__ = [
    "DEFAULT_PROFILES",
    "AllowedTransition",
    "AlgorithmMetrics",
    "AlgorithmTraceStep",
    "Edge",
    "FlowLite",
    "FrontierEntry",
    "GraphCache",
    "GraphProfile",
    "GraphResult",
    "MergedWindow",
    "Node",
    "ProfileCompatibility",
    "ProfileMatrixSummary",
    "RankExplanation",
    "ReceiptAdjustment",
    "ReadLoopGuard",
    "SourceSinkFrontier",
    "TypedPath",
    "TypedGraph",
    "ontology_catalog_to_graph_packet",
    "ontology_catalog_to_graph_request",
    "rank_frontier",
    "render_compact",
    "result_to_packet",
]


def __getattr__(name: str) -> object:
    if name not in _TURBO_EXPORTS:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    if name in {
        "ontology_catalog_to_graph_packet",
        "ontology_catalog_to_graph_request",
    }:
        from .ontology import (
            ontology_catalog_to_graph_packet,
            ontology_catalog_to_graph_request,
        )

        exports = {
            "ontology_catalog_to_graph_packet": ontology_catalog_to_graph_packet,
            "ontology_catalog_to_graph_request": ontology_catalog_to_graph_request,
        }
        value = exports[name]
        globals()[name] = value
        return value
    from .turbo import DEFAULT_PROFILES, rank_frontier, render_compact, result_to_packet

    exports = {
        "DEFAULT_PROFILES": DEFAULT_PROFILES,
        "rank_frontier": rank_frontier,
        "render_compact": render_compact,
        "result_to_packet": result_to_packet,
    }
    value = exports[name]
    globals()[name] = value
    return value
