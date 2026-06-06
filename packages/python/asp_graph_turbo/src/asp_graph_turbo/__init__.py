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
    RankExplanation,
    SourceSinkFrontier,
    TypedPath,
    TypedGraph,
)

_TURBO_EXPORTS = {
    "DEFAULT_PROFILES",
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
    "RankExplanation",
    "SourceSinkFrontier",
    "TypedPath",
    "TypedGraph",
    "rank_frontier",
    "render_compact",
    "result_to_packet",
]


def __getattr__(name: str) -> object:
    if name not in _TURBO_EXPORTS:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
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
