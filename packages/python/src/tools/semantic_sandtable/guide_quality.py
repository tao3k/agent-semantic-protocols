"""Agent guide-quality validation for sandtable expectations."""

from __future__ import annotations

import json
from typing import Any

from .models import StepResult
from .utils import string_list


def validate_guide_quality(
    expect: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    guide = expect.get("guideQuality")
    if guide is None:
        return
    if not isinstance(guide, dict):
        result.errors.append("expect.guideQuality must be an object")
        return

    _validate_source_leak_expectations(guide, result, stdout)
    decision = _guide_decision(guide, result, stdout)
    if decision is None:
        return
    _validate_decision_fields(guide, result, decision)
    routes = _decision_routes(decision)
    _validate_route_expectations(guide, result, routes, decision, stdout)


def _validate_source_leak_expectations(
    guide: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    for needle in string_list(guide.get("sourceLeakNotContains", [])):
        if needle in stdout:
            result.errors.append(f"guide leaked source text {needle!r}")


def _guide_decision(
    guide: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> dict[str, Any] | None:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as error:
        result.errors.append(f"guideQuality JSON parse failed: {error.msg}")
        return None

    decision = payload.get("agentHookDecision", payload)
    if not isinstance(decision, dict):
        result.errors.append("guideQuality missing agentHookDecision object")
        return None
    return decision


def _validate_decision_fields(
    guide: dict[str, Any],
    result: StepResult,
    decision: dict[str, Any],
) -> None:
    reason_kind = guide.get("reasonKind")
    if isinstance(reason_kind, str) and decision.get("reasonKind") != reason_kind:
        result.errors.append(
            f"guide reasonKind={decision.get('reasonKind')!r} expected={reason_kind!r}"
        )

    language_id = guide.get("languageId")
    language_ids = decision.get("languageIds", [])
    if isinstance(language_id, str):
        if not isinstance(language_ids, list) or language_id not in language_ids:
            result.errors.append(f"guide missing languageId {language_id!r}")


def _decision_routes(decision: dict[str, Any]) -> list[Any]:
    routes = decision.get("routes", [])
    if not isinstance(routes, list):
        return []
    return routes


def _validate_route_expectations(
    guide: dict[str, Any],
    result: StepResult,
    routes: list[Any],
    decision: dict[str, Any],
    stdout: str,
) -> None:
    route_kind = guide.get("routeKind")
    if isinstance(route_kind, str) and not _route_with_kind(routes, route_kind):
        result.errors.append(f"guide missing route kind {route_kind!r}")

    route_text = json.dumps(routes, sort_keys=True)
    message = decision.get("message", "")
    guide_text = "\n".join(
        [route_text, message if isinstance(message, str) else "", stdout]
    )
    for needle in string_list(guide.get("commandContains", [])):
        if needle not in guide_text:
            result.errors.append(f"guide missing command text {needle!r}")

    if bool(guide.get("requiresIngestPipe", False)) and not _has_ingest_pipe_route(
        routes
    ):
        result.errors.append("guide missing ingest pipe route")


def _route_with_kind(routes: list[Any], expected_kind: str) -> bool:
    return any(
        isinstance(route, dict) and route.get("kind") == expected_kind
        for route in routes
    )


def _has_ingest_pipe_route(routes: list[Any]) -> bool:
    for route in routes:
        if not isinstance(route, dict) or route.get("kind") != "ingest":
            continue
        argv = route.get("argv", [])
        stdin_mode = route.get("stdinMode")
        if isinstance(argv, list) and "search" in argv and "ingest" in argv:
            return True
        if stdin_mode == "pipe-candidates":
            return True
    return False
