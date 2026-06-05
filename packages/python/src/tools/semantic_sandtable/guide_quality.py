"""Agent guide-quality validation for sandtable expectations."""

from __future__ import annotations

import json
from typing import Any

from .models import StepResult
from .utils import string_list


_REASONING_PROFILE_NAMES = {
    "owner-query",
    "query-deps",
    "owner-tests",
    "finding-frontier",
    "feature-cfg",
}


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
    _validate_output_expectations(guide, result, stdout)
    _validate_graph_drift_expectations(result, stdout)
    _validate_prime_output_expectations(guide, result, stdout)
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


def _validate_output_expectations(
    guide: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    for needle in string_list(guide.get("outputContains", [])):
        if needle not in stdout:
            result.errors.append(f"guide output missing text {needle!r}")
    for needle in string_list(guide.get("outputNotContains", [])):
        if needle in stdout:
            result.errors.append(f"guide output contains stale text {needle!r}")


def _validate_graph_drift_expectations(result: StepResult, stdout: str) -> None:
    for needle in ("reasoning-selector", "finding(finding("):
        if needle in stdout:
            result.errors.append(f"guide output contains graph drift text {needle!r}")


def _validate_prime_output_expectations(
    guide: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    prime_output = guide.get("primeOutput")
    if prime_output is None:
        return
    if not isinstance(prime_output, dict):
        result.errors.append("expect.guideQuality.primeOutput must be an object")
        return

    _validate_prime_structure_status(prime_output, result, stdout)
    _validate_prime_entries(prime_output, result, stdout)


def _validate_prime_structure_status(
    prime_output: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    if not bool(prime_output.get("requiresStructureStatus", False)):
        return
    for needle in (
        "analysis=structure",
        "nativeSyntaxFacts=skipped",
        "policyFindings=skipped",
    ):
        if needle not in stdout:
            result.errors.append(f"guide prime output missing status field {needle!r}")


def _validate_prime_entries(
    prime_output: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    for entry in string_list(prime_output.get("entries", [])):
        _validate_prime_entry(entry, result, stdout)


def _validate_prime_entry(entry: str, result: StepResult, stdout: str) -> None:
    if not entry.startswith("entries="):
        result.errors.append(
            f"guide prime output entry must start with 'entries=': {entry!r}"
        )
        return
    _validate_prime_entry_profiles(entry, result)
    if entry not in stdout:
        result.errors.append(f"guide prime output missing entry {entry!r}")


def _validate_prime_entry_profiles(entry: str, result: StepResult) -> None:
    for profile_name in _entry_profile_names(entry):
        if profile_name not in _REASONING_PROFILE_NAMES:
            result.errors.append(
                f"guide prime output entry profile {profile_name!r} is not in the shared reasoning profile catalog"
            )


def _entry_profile_names(entry_line: str) -> list[str]:
    if not entry_line.startswith("entries=") or entry_line == "entries=":
        return []
    segments: list[str] = []
    segment = []
    depth = 0
    for char in entry_line.removeprefix("entries="):
        if char == "(":
            depth += 1
        elif char == ")" and depth > 0:
            depth -= 1
        if char == "," and depth == 0:
            segments.append("".join(segment))
            segment = []
            continue
        segment.append(char)
    if segment:
        segments.append("".join(segment))
    return [segment.split("(", 1)[0] for segment in segments if "(" in segment]

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

    decision = _decision_from_payload(payload)
    if not isinstance(decision, dict):
        result.errors.append("guideQuality missing agentHookDecision object")
        return None
    return decision


def _decision_from_payload(payload: Any) -> dict[str, Any] | None:
    if not isinstance(payload, dict):
        return None
    direct = payload.get("agentHookDecision")
    if isinstance(direct, dict):
        return direct
    embedded = payload.get("hookSpecificOutput", {}).get("additionalContext")
    if not isinstance(embedded, str):
        return payload
    found, decision = _decision_from_additional_context(embedded)
    if found:
        return decision
    return payload


def _decision_from_additional_context(
    embedded: str,
) -> tuple[bool, dict[str, Any] | None]:
    prefix = "[agent-hook-decision] "
    for line in embedded.splitlines():
        if not line.startswith(prefix):
            continue
        try:
            decision = json.loads(line[len(prefix) :])
        except json.JSONDecodeError:
            return True, None
        return True, decision if isinstance(decision, dict) else None
    return False, None


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

    route_command_text = _route_command_text(routes)
    for needle in string_list(guide.get("routeCommandContains", [])):
        if needle not in route_command_text:
            result.errors.append(f"guide missing route command text {needle!r}")
    for needle in string_list(guide.get("routeCommandNotContains", [])):
        if needle in route_command_text:
            result.errors.append(f"guide route contains stale command text {needle!r}")

    if bool(guide.get("requiresIngestPipe", False)) and not _has_ingest_pipe_route(
        routes
    ):
        result.errors.append("guide missing ingest pipe route")


def _route_with_kind(routes: list[Any], expected_kind: str) -> bool:
    return any(
        isinstance(route, dict) and route.get("kind") == expected_kind
        for route in routes
    )


def _route_command_text(routes: list[Any]) -> str:
    commands: list[str] = []
    for route in routes:
        if not isinstance(route, dict):
            continue
        argv = route.get("argv", [])
        if isinstance(argv, list):
            commands.append(" ".join(str(part) for part in argv))
    return "\n".join(commands)


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
