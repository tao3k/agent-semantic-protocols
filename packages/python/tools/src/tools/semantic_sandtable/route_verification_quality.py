"""Quality findings for route verification traces."""

from __future__ import annotations

from typing import Any

from .route_verification_common import (
    ROUTE_IDS,
    SCORE_KEYS,
    finding,
    first_command_id,
    kebab,
    list_of_dicts,
    risk_command_refs,
    route_command_refs,
    score_value,
    string_set,
)
from .utils import dict_value, string_list


def quality_findings(
    trace: dict[str, Any],
    expectation: dict[str, Any] | None = None,
) -> list[dict[str, Any]]:
    expected = dict_value(expectation)
    if not expected:
        return []
    context = _quality_context(trace)
    findings: list[dict[str, Any]] = []
    findings.extend(_first_route_findings(trace, expected, context))
    findings.extend(_forbidden_route_findings(trace, expected, context))
    findings.extend(_missing_rejected_route_findings(trace, expected, context))
    findings.extend(_forbidden_risk_findings(trace, expected, context))
    findings.extend(_score_findings(expected, context))
    findings.extend(_requirement_findings(trace, expected, context))
    return findings


def _quality_context(trace: dict[str, Any]) -> dict[str, Any]:
    executed_routes = [
        str(step.get("route"))
        for step in list_of_dicts(trace.get("executedTrace"))
        if str(step.get("route")) in ROUTE_IDS
    ]
    return {
        "executedRoutes": executed_routes,
        "firstRoute": executed_routes[0] if executed_routes else None,
        "riskKinds": [
            str(flag.get("kind"))
            for flag in list_of_dicts(trace.get("riskFlags"))
            if isinstance(flag.get("kind"), str)
        ],
        "rejectedRoutes": {
            str(route.get("route"))
            for route in list_of_dicts(trace.get("rejectedRoutes"))
            if isinstance(route.get("route"), str)
        },
        "scores": dict_value(trace.get("behaviorScores")),
    }


def _first_route_findings(
    trace: dict[str, Any],
    expected: dict[str, Any],
    context: dict[str, Any],
) -> list[dict[str, Any]]:
    first_route = context["firstRoute"]
    allowed_first = string_set(expected.get("allowedFirstRoutes"))
    if not first_route or not allowed_first or first_route in allowed_first:
        return []
    return [
        finding(
            f"route.first-route.{first_route}",
            "route-verification",
            "warning",
            f"First route was {first_route}, outside the allowed first routes.",
            "Start from the narrowest route allowed by the current evidence state.",
            evidence_refs=[f"command:{first_command_id(trace)}"],
        )
    ]


def _forbidden_route_findings(
    trace: dict[str, Any],
    expected: dict[str, Any],
    context: dict[str, Any],
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    executed_routes = context["executedRoutes"]
    for route in string_list(expected.get("forbiddenRoutes")):
        if route not in executed_routes:
            continue
        findings.append(
            finding(
                f"route.forbidden.{route}",
                "route-verification",
                "error",
                f"Forbidden route {route} was executed.",
                "Replace this route with an owner/query/AST route justified by evidence state.",
                evidence_refs=route_command_refs(trace, route),
            )
        )
    return findings


def _missing_rejected_route_findings(
    trace: dict[str, Any],
    expected: dict[str, Any],
    context: dict[str, Any],
) -> list[dict[str, Any]]:
    del trace
    findings: list[dict[str, Any]] = []
    rejected_routes = context["rejectedRoutes"]
    for route in string_list(expected.get("requiredRejectedRoutes")):
        if route in rejected_routes:
            continue
        findings.append(
            finding(
                f"route.missing-rejection.{route}",
                "route-verification",
                "warning",
                f"Route {route} was not recorded as rejected.",
                "Record rejected broad or unsafe routes with the reason they were skipped.",
            )
        )
    return findings


def _forbidden_risk_findings(
    trace: dict[str, Any],
    expected: dict[str, Any],
    context: dict[str, Any],
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    risk_kinds = context["riskKinds"]
    for risk in string_list(expected.get("forbiddenRiskFlags")):
        if risk not in risk_kinds:
            continue
        findings.append(
            finding(
                f"route.risk.{risk}",
                "route-verification",
                "error",
                f"Forbidden route risk {risk} was observed.",
                "Adjust the search flow so this risk cannot occur for this evidence state.",
                evidence_refs=risk_command_refs(trace, risk),
            )
        )
    return findings


def _score_findings(
    expected: dict[str, Any],
    context: dict[str, Any],
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    scores = context["scores"]
    for score_name, minimum in dict_value(expected.get("minScores")).items():
        if score_name not in SCORE_KEYS:
            continue
        actual = score_value(scores.get(score_name))
        required = score_value(minimum)
        if actual >= required:
            continue
        findings.append(
            finding(
                f"route.score.{kebab(score_name)}",
                "route-verification",
                "warning",
                f"{score_name} score {actual} is below required minimum {required}.",
                "Improve route selection until the verifier score reaches the scenario floor.",
            )
        )
    return findings


def _requirement_findings(
    trace: dict[str, Any],
    expected: dict[str, Any],
    context: dict[str, Any],
) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    if bool(expected.get("requireRouteJustification")) and not _has_justification(trace):
        findings.append(_missing_justification_finding())
    if bool(expected.get("requireExactCodeIdentity")) and not _has_exact_code_identity(
        trace
    ):
        findings.append(_missing_exact_code_identity_finding())
    if bool(expected.get("requireNoExecutableLineRange")):
        findings.extend(_line_range_requirement_findings(trace, context))
    if bool(expected.get("requireVerificationEvidence")):
        if not _has_verification_evidence(trace):
            findings.append(_missing_verification_evidence_finding())
    return findings


def _line_range_requirement_findings(
    trace: dict[str, Any],
    context: dict[str, Any],
) -> list[dict[str, Any]]:
    if "executable-line-range" not in context["riskKinds"]:
        return []
    return [
        finding(
            "route.executable-line-range",
            "route-verification",
            "error",
            "A line range was used as an executable selector.",
            "Keep line ranges as display hints and execute parser-owned structural selectors.",
            evidence_refs=risk_command_refs(trace, "executable-line-range"),
        )
    ]


def _has_justification(trace: dict[str, Any]) -> bool:
    chosen = dict_value(trace.get("chosenRoute"))
    if not chosen.get("reason"):
        return False
    return all(plan.get("reason") for plan in list_of_dicts(trace.get("routePlan")))


def _has_exact_code_identity(trace: dict[str, Any]) -> bool:
    return any(
        step.get("route") == "query-code" and step.get("codePolicy") == "exact-only"
        for step in list_of_dicts(trace.get("executedTrace"))
    )


def _has_verification_evidence(trace: dict[str, Any]) -> bool:
    chosen = dict_value(trace.get("chosenRoute"))
    if string_list(chosen.get("evidenceRefs")):
        return True
    return any(
        string_list(step.get("evidenceRefs"))
        for step in list_of_dicts(trace.get("executedTrace"))
    )


def _missing_justification_finding() -> dict[str, Any]:
    return finding(
        "route.missing-justification",
        "route-verification",
        "error",
        "Route justification is missing from the trace.",
        "Emit chosen route and route plan reasons before executing follow-up searches.",
    )


def _missing_exact_code_identity_finding() -> dict[str, Any]:
    return finding(
        "route.missing-exact-code-identity",
        "route-verification",
        "warning",
        "No exact query-code step was recorded before code-level evidence.",
        "Use parser-owned query-code with an exact structural selector before code reads.",
    )


def _missing_verification_evidence_finding() -> dict[str, Any]:
    return finding(
        "route.missing-verification-evidence",
        "route-verification",
        "warning",
        "Route verification trace has no command evidence references.",
        "Attach command ids or packet ids to chosen route and executed steps.",
    )
