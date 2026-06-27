"""Build route verification trace packets from recorded commands."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from .direct_read_shape import direct_source_read_shape
from .route_verification_common import (
    ANCHORS_FOR_PRECISE_ROUTING,
    BROAD_SEARCH_ROUTES,
    LINE_RANGE_RE,
    ROUTE_IDS,
    SCORE_KEYS,
    argv,
    command_id,
    command_refs,
    index,
)
from .utils import dict_value, string_list


def build_trace(
    commands: list[dict[str, Any]],
    expectation: dict[str, Any] | None = None,
) -> dict[str, Any]:
    expected = dict_value(expectation)
    executed = _executed_trace(commands)
    anchors = _evidence_anchors(commands, expected)
    chosen_route = executed[0]["route"] if executed else _planned_first_route(expected)
    risk_flags = _risk_flags(commands, executed, anchors, expected)
    behavior_scores = _behavior_scores(executed, risk_flags, expected)
    trace = {
        "schemaId": "agent.semantic-protocols.semantic-route-verification-trace",
        "schemaVersion": "1",
        "verifierVersion": "asp-route-verifier.v1",
        "monitorPatternSetVersion": "2026-06-27",
        "evidenceState": _evidence_state(commands, anchors),
        "chosenRoute": {
            "route": chosen_route,
            "reason": _chosen_route_reason(chosen_route, anchors),
            "evidenceRefs": command_refs(executed[:1]),
        },
        "rejectedRoutes": _rejected_routes(anchors, expected),
        "routePlan": _route_plan(anchors, expected, chosen_route),
        "executedTrace": executed,
        "behaviorScores": behavior_scores,
        "riskFlags": risk_flags,
        "routeRegret": max(0, 4 - min(behavior_scores.values())),
    }
    feedback = _feedback_signals(risk_flags)
    if feedback:
        trace["feedbackSignals"] = feedback
    return trace


def _executed_trace(commands: list[dict[str, Any]]) -> list[dict[str, Any]]:
    executed: list[dict[str, Any]] = []
    for item_index, command in enumerate(commands, start=1):
        command_argv = argv(command)
        route = _route_id(command_argv)
        if route is None:
            continue
        safe_id = command_id(command, item_index)
        executed.append(
            {
                "commandId": safe_id,
                "route": route,
                "projection": _projection(command_argv, route),
                "codePolicy": _code_policy(command_argv, route),
                "evidenceRefs": [f"command:{safe_id}"],
            }
        )
    return executed


def _evidence_state(
    commands: list[dict[str, Any]],
    anchors: list[str],
) -> dict[str, Any]:
    state: dict[str, Any] = {"anchors": anchors}
    owner_path = _owner_path(commands)
    if owner_path:
        state["ownerPath"] = owner_path
    query = _query_text(commands)
    if query:
        state["query"] = query
    refs = _route_command_refs(commands)
    if refs:
        state["evidenceRefs"] = refs[:5]
    return state


def _evidence_anchors(
    commands: list[dict[str, Any]],
    expectation: dict[str, Any],
) -> list[str]:
    anchors: list[str] = []
    for anchor in string_list(expectation.get("expectedEvidenceAnchors")):
        if anchor not in anchors:
            anchors.append(anchor)
    if _owner_path(commands) and "owner-path" not in anchors:
        anchors.append("owner-path")
    if _has_semantic_selector(commands) and "exact-selector" not in anchors:
        anchors.append("exact-selector")
    if _query_text(commands) and "user-anchor" not in anchors:
        anchors.append("user-anchor")
    return anchors or ["unknown-workspace"]


def _route_command_refs(commands: list[dict[str, Any]]) -> list[str]:
    refs: list[str] = []
    for item_index, command in enumerate(commands, start=1):
        if _route_id(argv(command)) is not None:
            refs.append(f"command:{command_id(command, item_index)}")
    return refs


def _risk_flags(
    commands: list[dict[str, Any]],
    executed: list[dict[str, Any]],
    anchors: list[str],
    expectation: dict[str, Any],
) -> list[dict[str, Any]]:
    risks: list[dict[str, Any]] = []
    route_counts: dict[str, int] = {}
    precise = bool(set(anchors) & ANCHORS_FOR_PRECISE_ROUTING)
    allowed_first = set(string_list(expectation.get("allowedFirstRoutes")))
    for item_index, command in enumerate(commands, start=1):
        _collect_command_risks(
            risks,
            route_counts,
            argv(command),
            command_id(command, item_index),
            precise=precise,
            allowed_first=allowed_first,
            expectation=expectation,
        )
    _collect_repeated_broad_search_risks(risks, route_counts, executed)
    if not executed and bool(expectation.get("requireVerificationEvidence")):
        _append_risk(
            risks,
            "unsupported-verification-claim",
            "warning",
            "verification was required but no route command was recorded.",
            None,
        )
    return risks


def _collect_command_risks(
    risks: list[dict[str, Any]],
    route_counts: dict[str, int],
    command_argv: list[str],
    safe_id: str,
    *,
    precise: bool,
    allowed_first: set[str],
    expectation: dict[str, Any],
) -> None:
    route = _route_id(command_argv)
    if route is None:
        return
    route_counts[route] = route_counts.get(route, 0) + 1
    if route == "prime" and precise and "prime" not in allowed_first:
        _append_risk(
            risks,
            "unnecessary-prime",
            "warning",
            "search prime ran even though narrower evidence anchors were available.",
            safe_id,
        )
    if route == "direct-read" and (
        precise or bool(expectation.get("requireExactCodeIdentity"))
    ):
        _append_risk(
            risks,
            "direct-read-over-parser",
            "warning",
            "direct-source-read was used where parser-owned query routes were expected.",
            safe_id,
        )
    _collect_line_selector_risks(risks, command_argv, safe_id)


def _collect_line_selector_risks(
    risks: list[dict[str, Any]],
    command_argv: list[str],
    safe_id: str,
) -> None:
    if _has_line_range_selector(command_argv):
        _append_risk(
            risks,
            "executable-line-range",
            "error",
            "a path line range appeared in an executable selector position.",
            safe_id,
        )
    elif _has_hidden_line_range(command_argv):
        _append_risk(
            risks,
            "hidden-line-selector",
            "warning",
            "a path line range appeared outside display-only fields.",
            safe_id,
        )


def _collect_repeated_broad_search_risks(
    risks: list[dict[str, Any]],
    route_counts: dict[str, int],
    executed: list[dict[str, Any]],
) -> None:
    for route in sorted(BROAD_SEARCH_ROUTES):
        if route_counts.get(route, 0) <= 1:
            continue
        _append_risk(
            risks,
            "repeated-broad-search",
            "warning",
            f"route {route} repeated instead of narrowing the evidence state.",
            _first_command_for_route(executed, route),
        )


def _behavior_scores(
    executed: list[dict[str, Any]],
    risks: list[dict[str, Any]],
    expectation: dict[str, Any],
) -> dict[str, int]:
    risk_kinds = {str(risk.get("kind")) for risk in risks}
    executed_routes = [str(step.get("route")) for step in executed]
    scores = {key: 4 for key in SCORE_KEYS}
    _apply_route_score_penalties(scores, executed_routes, expectation)
    _apply_risk_score_penalties(scores, risk_kinds)
    if not executed:
        scores["finalAnswerGrounding"] -= 2
    return {key: max(0, min(4, value)) for key, value in scores.items()}


def _apply_route_score_penalties(
    scores: dict[str, int],
    executed_routes: list[str],
    expectation: dict[str, Any],
) -> None:
    forbidden_routes = set(string_list(expectation.get("forbiddenRoutes")))
    if forbidden_routes & set(executed_routes):
        scores["routeFaithfulness"] -= 2
    allowed_first = set(string_list(expectation.get("allowedFirstRoutes")))
    if allowed_first and executed_routes and executed_routes[0] not in allowed_first:
        scores["routeFaithfulness"] -= 1


def _apply_risk_score_penalties(scores: dict[str, int], risk_kinds: set[str]) -> None:
    if "unnecessary-prime" in risk_kinds:
        scores["routeEfficiency"] -= 2
    if "repeated-broad-search" in risk_kinds:
        scores["routeEfficiency"] -= 1
    if "direct-read-over-parser" in risk_kinds:
        scores["semanticPrecision"] -= 2
        scores["fallbackDiscipline"] -= 2
    if "executable-line-range" in risk_kinds:
        scores["semanticPrecision"] -= 2
        scores["fallbackDiscipline"] -= 2
    if "hidden-line-selector" in risk_kinds:
        scores["fallbackDiscipline"] -= 1
    if "unsupported-verification-claim" in risk_kinds:
        scores["verificationHonesty"] -= 2
        scores["finalAnswerGrounding"] -= 1


def _route_plan(
    anchors: list[str],
    expectation: dict[str, Any],
    chosen_route: str,
) -> list[dict[str, Any]]:
    routes = string_list(expectation.get("allowedFirstRoutes")) or [chosen_route]
    plan = [_planned_route(route, anchors) for route in routes if route in ROUTE_IDS]
    return plan or [
        {
            "route": "custom",
            "expectedProjection": "unknown",
            "codePolicy": "unknown",
            "reason": "no standard route matched the recorded command trace",
        }
    ]


def _planned_route(route: str, anchors: list[str]) -> dict[str, Any]:
    return {
        "route": route,
        "preconditions": [anchor for anchor in anchors if anchor != "unknown-workspace"],
        "expectedProjection": _expected_projection_for_route(route),
        "codePolicy": _expected_code_policy_for_route(route),
        "reason": _planned_route_reason(route, anchors),
    }


def _rejected_routes(
    anchors: list[str],
    expectation: dict[str, Any],
) -> list[dict[str, Any]]:
    rejected = [
        _rejected_route(route, anchors)
        for route in string_list(expectation.get("requiredRejectedRoutes"))
        if route in ROUTE_IDS
    ]
    rejected_routes = {item["route"] for item in rejected}
    precise = bool(set(anchors) & ANCHORS_FOR_PRECISE_ROUTING)
    if precise and "prime" not in rejected_routes:
        rejected.append(_rejected_route("prime", anchors))
    if bool(expectation.get("requireNoExecutableLineRange")):
        if "direct-read" not in {item["route"] for item in rejected}:
            rejected.append(_rejected_route("direct-read", anchors))
    return rejected


def _rejected_route(route: str, anchors: list[str]) -> dict[str, str]:
    if route == "prime":
        return {
            "route": "prime",
            "reason": "precise evidence anchors exist, so workspace primer is broader than necessary",
            "risk": "unnecessary-prime",
        }
    if route == "direct-read":
        return {
            "route": "direct-read",
            "reason": "line ranges are display hints unless a parser route cannot provide exact identity",
            "risk": "executable-line-range",
        }
    return {
        "route": route,
        "reason": f"route is not justified by anchors {', '.join(anchors)}",
    }


def _route_id(command_argv: list[str]) -> str | None:
    if not command_argv:
        return None
    if _has_direct_source_read(command_argv):
        return "direct-read"
    if "query" in command_argv:
        return "query-code"
    asp_index = _asp_index(command_argv)
    if asp_index is not None and asp_index + 1 < len(command_argv):
        direct_route = _direct_asp_route(command_argv[asp_index + 1])
        if direct_route:
            return direct_route
    search_index = index(command_argv, "search")
    if search_index is None or search_index + 1 >= len(command_argv):
        return None
    return _search_route(command_argv[search_index + 1], command_argv[search_index + 2 :])


def _direct_asp_route(verb: str) -> str | None:
    if verb == "fd":
        return "fd-query"
    if verb == "rg":
        return "rg-query"
    return None


def _search_route(subcommand: str, args: list[str]) -> str | None:
    if subcommand in {"prime", "pipe", "fzf"}:
        return subcommand
    if subcommand in {"fd", "finder"}:
        return "fd-query"
    if subcommand in {"rg", "grep"}:
        return "rg-query"
    if subcommand == "owner":
        return "owner-items"
    if subcommand in {"dependency", "deps", "public-external-types"}:
        return "dependency-topology"
    if subcommand == "tests":
        return "test-frontier"
    if subcommand == "reasoning":
        return _reasoning_route(args)
    if subcommand in {"symbol", "callsite", "import", "api"}:
        return "owner-items"
    return None


def _reasoning_route(args: list[str]) -> str:
    if any(arg in {"owner-tests", "tests"} for arg in args):
        return "test-frontier"
    if any(arg in {"query-deps", "dependency", "deps"} for arg in args):
        return "dependency-topology"
    return "owner-items"


def _projection(command_argv: list[str], route: str) -> str:
    if "--json" in command_argv:
        return "json"
    if "--code" in command_argv:
        return "code"
    if "--names-only" in command_argv:
        return "names"
    view_index = index(command_argv, "--view")
    if view_index is not None and view_index + 1 < len(command_argv):
        view = command_argv[view_index + 1]
        if view in {"seeds", "json"}:
            return view
    return _default_projection(route)


def _default_projection(route: str) -> str:
    if route in {"prime", "pipe", "fzf", "fd-query", "rg-query"}:
        return "seeds"
    if route == "owner-items":
        return "names"
    if route in {"owner-skeleton", "syntax-outline"}:
        return "outline"
    if route == "query-code":
        return "code"
    return "unknown"


def _code_policy(command_argv: list[str], route: str) -> str:
    if route == "query-code":
        return "exact-only" if "--code" in command_argv else "unknown"
    if route == "direct-read":
        shape = direct_source_read_shape(command_argv)
        return "fallback-bounded" if shape == "bounded" else "unknown"
    return "disabled"


def _expected_projection_for_route(route: str) -> str:
    if route == "query-code":
        return "code"
    if route == "owner-items":
        return "names"
    if route in {"owner-skeleton", "syntax-outline"}:
        return "outline"
    if route in {"prime", "pipe", "fzf", "fd-query", "rg-query"}:
        return "seeds"
    return "unknown"


def _expected_code_policy_for_route(route: str) -> str:
    if route == "query-code":
        return "exact-only"
    if route == "direct-read":
        return "fallback-bounded"
    return "disabled"


def _chosen_route_reason(route: str, anchors: list[str]) -> str:
    if route in {"owner-items", "query-code"}:
        return f"route follows precise evidence anchors: {', '.join(anchors)}"
    if route == "prime":
        return "route starts from project-level discovery"
    if route in {"fd-query", "rg-query", "fzf", "pipe"}:
        return "route uses query terms before selecting parser-owned evidence"
    return "route reconstructed from recorded command trace"


def _planned_route_reason(route: str, anchors: list[str]) -> str:
    if route == "query-code":
        return "read code only after exact parser identity is available"
    if route == "owner-items":
        return "inspect owner-local items from the known evidence path"
    if route == "prime":
        return "use only when no narrower owner, selector, or dependency evidence exists"
    return f"route is allowed for anchors {', '.join(anchors)}"


def _planned_first_route(expectation: dict[str, Any]) -> str:
    allowed = string_list(expectation.get("allowedFirstRoutes"))
    return allowed[0] if allowed and allowed[0] in ROUTE_IDS else "custom"


def _has_direct_source_read(command_argv: list[str]) -> bool:
    return "--from-hook" in command_argv and "direct-source-read" in command_argv


def _asp_index(command_argv: list[str]) -> int | None:
    for item_index, value in enumerate(command_argv):
        binary = Path(value).name
        if binary == "asp" or binary.endswith("-harness"):
            return item_index
    return None


def _has_semantic_selector(commands: list[dict[str, Any]]) -> bool:
    return any(
        "://" in selector
        for command in commands
        for selector in _selectors(argv(command))
    )


def _has_line_range_selector(command_argv: list[str]) -> bool:
    return any(_looks_like_line_range(selector) for selector in _selectors(command_argv))


def _has_hidden_line_range(command_argv: list[str]) -> bool:
    selector_values = set(_selectors(command_argv))
    return any(
        _looks_like_line_range(token)
        for token in command_argv
        if token not in selector_values and not token.startswith("--")
    )


def _selectors(command_argv: list[str]) -> list[str]:
    values: list[str] = []
    for item_index, value in enumerate(command_argv):
        if value == "--selector" and item_index + 1 < len(command_argv):
            values.append(command_argv[item_index + 1])
    return values


def _looks_like_line_range(value: str) -> bool:
    return "://" not in value and bool(LINE_RANGE_RE.search(value))


def _owner_path(commands: list[dict[str, Any]]) -> str | None:
    for command in commands:
        owner = _owner_path_from_argv(argv(command))
        if owner:
            return owner
    return None


def _owner_path_from_argv(command_argv: list[str]) -> str | None:
    search_index = index(command_argv, "search")
    if search_index is not None and search_index + 2 < len(command_argv):
        if command_argv[search_index + 1] == "owner":
            owner = _project_path_or_none(command_argv[search_index + 2])
            if owner:
                return owner
    for selector in _selectors(command_argv):
        owner = _project_path_or_none(_selector_owner_path(selector))
        if owner:
            return owner
    return None


def _selector_owner_path(selector: str) -> str:
    if "://" in selector:
        return ""
    return LINE_RANGE_RE.sub("", selector)


def _project_path_or_none(value: str) -> str | None:
    if not value or value.startswith("-") or value.startswith("/") or "://" in value:
        return None
    if re.match(r"^[A-Za-z]:[\\/]", value):
        return None
    return value


def _query_text(commands: list[dict[str, Any]]) -> str | None:
    for command in commands:
        command_argv = argv(command)
        for flag in ("--query", "-query"):
            flag_index = index(command_argv, flag)
            if flag_index is not None and flag_index + 1 < len(command_argv):
                return command_argv[flag_index + 1]
    return None


def _append_risk(
    risks: list[dict[str, Any]],
    kind: str,
    severity: str,
    message: str,
    safe_id: str | None,
) -> None:
    for risk in risks:
        if risk.get("kind") == kind and risk.get("commandId") == safe_id:
            return
    risk: dict[str, Any] = {"kind": kind, "severity": severity, "message": message}
    if safe_id:
        risk["commandId"] = safe_id
        risk["evidenceRefs"] = [f"command:{safe_id}"]
    risks.append(risk)


def _first_command_for_route(
    executed: list[dict[str, Any]],
    route: str,
) -> str | None:
    for step in executed:
        if step.get("route") == route and isinstance(step.get("commandId"), str):
            return str(step["commandId"])
    return None


def _feedback_signals(risks: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        signal
        for risk in risks
        if (signal := _feedback_signal(risk)) is not None
    ]


def _feedback_signal(risk: dict[str, Any]) -> dict[str, Any] | None:
    reason = _feedback_reason(str(risk.get("kind")))
    if reason is None:
        return None
    signal: dict[str, Any] = {
        "reason": reason,
        "polarity": "negative",
        "confidence": _feedback_confidence(str(risk.get("severity"))),
    }
    refs = risk.get("evidenceRefs")
    if isinstance(refs, list) and all(isinstance(item, str) for item in refs):
        signal["evidenceRefs"] = refs
    return signal


def _feedback_reason(risk_kind: str) -> str | None:
    if risk_kind in {"unnecessary-prime", "repeated-broad-search"}:
        return "inefficiency"
    if risk_kind in {
        "direct-read-over-parser",
        "executable-line-range",
        "hidden-line-selector",
    }:
        return "overaction"
    if risk_kind == "unsupported-verification-claim":
        return "communication"
    return None


def _feedback_confidence(severity: str) -> float:
    if severity == "error":
        return 0.95
    if severity == "warning":
        return 0.85
    return 0.65
