"""Versioned route monitor patterns used by route verification."""

from __future__ import annotations

from typing import Any


MONITOR_PATTERN_SET_VERSION = "2026-06-27"
USER_FEEDBACK_DATASET_VERSION = "2026-06-27"

ROUTE_MONITOR_PATTERNS: tuple[dict[str, Any], ...] = (
    {
        "id": "route.unnecessary-prime",
        "riskKind": "unnecessary-prime",
        "feedbackReason": "inefficiency",
        "severity": "warning",
        "trigger": {
            "routes": ["prime"],
            "evidenceAnchors": [
                "exact-selector",
                "structural-selector",
                "owner-path",
                "symbol",
                "changed-file",
            ],
        },
        "description": "search prime ran after a narrower evidence anchor existed.",
        "addedFrom": "user-feedback",
        "evidenceRefs": ["route-feedback:avoid-prime-when-owner-known"],
        "status": "active",
    },
    {
        "id": "route.repeated-broad-search",
        "riskKind": "repeated-broad-search",
        "feedbackReason": "inefficiency",
        "severity": "warning",
        "trigger": {
            "routes": ["prime", "fd-query", "rg-query"],
            "minimumOccurrences": 2,
        },
        "description": "broad search repeated instead of narrowing the evidence state.",
        "addedFrom": "sandtable",
        "status": "active",
    },
    {
        "id": "graph.owner-only-frontier-redundancy",
        "riskKind": "owner-only-frontier-redundancy",
        "feedbackReason": "inefficiency",
        "severity": "warning",
        "trigger": {
            "outputShape": "owner-topology-only-graph",
            "redundantFields": [
                "rank",
                "frontier",
                "rankedEvidence",
                "evidenceFrontier",
            ],
        },
        "description": "owner/topology-only graph output repeated rank or frontier fields.",
        "addedFrom": "user-feedback",
        "evidenceRefs": ["route-feedback:owner-only-frontier-is-redundant"],
        "status": "active",
    },
    {
        "id": "route.direct-read-over-parser",
        "riskKind": "direct-read-over-parser",
        "feedbackReason": "overaction",
        "severity": "warning",
        "trigger": {
            "routes": ["direct-read"],
            "evidenceAnchors": [
                "exact-selector",
                "structural-selector",
                "owner-path",
                "symbol",
            ],
        },
        "description": "direct source read was used where parser-owned identity was available.",
        "addedFrom": "user-feedback",
        "evidenceRefs": ["route-feedback:parser-before-direct-read"],
        "status": "active",
    },
    {
        "id": "selector.executable-line-range",
        "riskKind": "executable-line-range",
        "feedbackReason": "overaction",
        "severity": "error",
        "trigger": {
            "selectorShape": "path-line-range",
            "selectorPosition": "executable",
        },
        "description": "a display line range was used as an executable selector.",
        "addedFrom": "user-feedback",
        "evidenceRefs": ["route-feedback:line-range-is-display-hint"],
        "status": "active",
    },
    {
        "id": "selector.hidden-line-range",
        "riskKind": "hidden-line-selector",
        "feedbackReason": "overaction",
        "severity": "warning",
        "trigger": {
            "selectorShape": "path-line-range",
            "selectorPosition": "hidden",
        },
        "description": "a line range appeared outside a display-only field.",
        "addedFrom": "sandtable",
        "status": "active",
    },
    {
        "id": "verification.unsupported-claim",
        "riskKind": "unsupported-verification-claim",
        "feedbackReason": "communication",
        "severity": "warning",
        "trigger": {
            "requiresVerificationEvidence": True,
            "minimumExecutedRoutes": 1,
        },
        "description": "verification was claimed without replayable route evidence.",
        "addedFrom": "sandtable",
        "status": "active",
    },
)


def feedback_reason_for_risk(risk_kind: str) -> str | None:
    pattern = monitor_pattern_for_risk(risk_kind)
    return str(pattern["feedbackReason"]) if pattern else None


def feedback_pattern_id_for_risk(risk_kind: str) -> str | None:
    pattern = monitor_pattern_for_risk(risk_kind)
    return str(pattern["id"]) if pattern else None


def feedback_refs_for_risk(risk_kind: str) -> list[str]:
    pattern = monitor_pattern_for_risk(risk_kind)
    if not pattern:
        return []
    refs = pattern.get("evidenceRefs")
    if not isinstance(refs, list):
        return []
    return [
        str(ref)
        for ref in refs
        if isinstance(ref, str) and ref.startswith("route-feedback:")
    ]


def monitor_pattern_for_risk(risk_kind: str) -> dict[str, Any] | None:
    for pattern in ROUTE_MONITOR_PATTERNS:
        if pattern["riskKind"] == risk_kind:
            return pattern
    return None


def feedback_confidence_for_severity(severity: str) -> float:
    if severity == "error":
        return 0.95
    if severity == "warning":
        return 0.85
    return 0.65
