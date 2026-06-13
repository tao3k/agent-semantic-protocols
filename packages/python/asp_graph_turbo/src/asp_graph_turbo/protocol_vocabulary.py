"""Shared semantic fact vocabulary boundaries for graph-turbo profiles."""

from __future__ import annotations

GRAPH_TURBO_INTERNAL_RELATIONS = frozenset(
    {
        "checks",
        "gates",
        "relates",
        "repairs",
        "selects",
        "split",
        "uses",
    }
)

ONTOLOGY_ONLY_RELATIONS = frozenset(
    {
        "calls",
        "imports",
        "mutates",
        "reads",
        "writes",
    }
)

FACT_GRAPH_ONLY_RELATIONS = frozenset(
    {
        "affects",
        "covered_by",
        "derived-from",
        "depends_on",
        "observed-by",
        "packages",
        "requires-evidence",
        "reviewed-by",
        "suggests-action",
        "supports-claim",
        "targets",
        "tests",
        "verified-by",
        "waived-by",
    }
)

ONTOLOGY_TO_FACT_GRAPH_CONFIDENCE = {
    "exact": frozenset({"exact"}),
    "inferred": frozenset({"high", "medium", "low"}),
    "heuristic": frozenset({"heuristic"}),
}

ONTOLOGY_TO_FACT_GRAPH_FRESHNESS = {
    "fresh": frozenset({"fresh", "cache-hit"}),
    "stale": frozenset({"stale"}),
    "unknown": frozenset({"unknown"}),
}
