"""Shared helpers for route verification receipts."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any

from .utils import string_list


ANCHORS_FOR_PRECISE_ROUTING = {
    "exact-selector",
    "structural-selector",
    "owner-path",
    "symbol",
    "changed-file",
}
BROAD_SEARCH_ROUTES = {"prime", "fd-query", "rg-query"}
LINE_RANGE_RE = re.compile(r":\d+(?::\d+)?(?:-\d+(?::\d+)?)?$")
SAFE_ID_RE = re.compile(r"[^a-z0-9_-]+")
ROUTE_IDS = {
    "prime",
    "pipe",
    "lexical",
    "fd-query",
    "rg-query",
    "owner-items",
    "owner-skeleton",
    "syntax-outline",
    "query-code",
    "dependency-topology",
    "failure-frontier",
    "test-frontier",
    "direct-read",
    "custom",
}
SCORE_KEYS = (
    "routeFaithfulness",
    "routeEfficiency",
    "semanticPrecision",
    "fallbackDiscipline",
    "verificationHonesty",
    "finalAnswerGrounding",
)


@dataclass(frozen=True)
class RouteVerificationResult:
    trace: dict[str, Any]
    quality_findings: list[dict[str, Any]]


def argv(command: dict[str, Any]) -> list[str]:
    value = command.get("argv")
    if not isinstance(value, list):
        return []
    return [str(part) for part in value]


def command_id(command: dict[str, Any], index: int) -> str:
    raw = str(command.get("id") or f"command-{index}").lower()
    normalized = SAFE_ID_RE.sub("-", raw).strip("-_")
    if not normalized:
        normalized = f"command-{index}"
    if not normalized[0].isalpha():
        normalized = f"command-{normalized}"
    return normalized


def list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def score_value(value: Any) -> int:
    try:
        score = int(value)
    except (TypeError, ValueError):
        return 0
    return max(0, min(4, score))


def index(items: list[str], value: str) -> int | None:
    try:
        return items.index(value)
    except ValueError:
        return None


def command_refs(steps: list[dict[str, Any]]) -> list[str]:
    return [f"command:{step['commandId']}" for step in steps if "commandId" in step]


def route_command_refs(trace: dict[str, Any], route: str) -> list[str]:
    return [
        f"command:{step.get('commandId')}"
        for step in list_of_dicts(trace.get("executedTrace"))
        if step.get("route") == route
    ]


def risk_command_refs(trace: dict[str, Any], risk_kind: str) -> list[str]:
    return [
        f"command:{risk['commandId']}"
        for risk in list_of_dicts(trace.get("riskFlags"))
        if risk.get("kind") == risk_kind and isinstance(risk.get("commandId"), str)
    ]


def first_command_id(trace: dict[str, Any]) -> str:
    for step in list_of_dicts(trace.get("executedTrace")):
        value = step.get("commandId")
        if isinstance(value, str):
            return value
    return "unknown"


def finding(
    finding_id: str,
    kind: str,
    severity: str,
    message: str,
    recommended_action: str,
    *,
    evidence_refs: list[str] | None = None,
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "id": finding_id,
        "kind": kind,
        "severity": severity,
        "message": message,
        "recommendedAction": recommended_action,
    }
    if evidence_refs:
        result["evidenceRefs"] = evidence_refs
    return result


def string_set(value: Any) -> set[str]:
    return set(string_list(value))


def kebab(value: str) -> str:
    return re.sub(r"(?<!^)([A-Z])", r"-\1", value).lower()
