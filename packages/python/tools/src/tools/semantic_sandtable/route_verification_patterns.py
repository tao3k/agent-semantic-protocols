"""Versioned route monitor patterns used by route verification."""

from __future__ import annotations

from typing import Any


MONITOR_PATTERN_SET_VERSION = "2026-06-27"

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
    for pattern in ROUTE_MONITOR_PATTERNS:
        if pattern["riskKind"] == risk_kind:
            return str(pattern["feedbackReason"])
    return None


def feedback_confidence_for_severity(severity: str) -> float:
    if severity == "error":
        return 0.95
    if severity == "warning":
        return 0.85
    return 0.65
