"""Dynamic route judge checklist for route verification traces."""

from __future__ import annotations

from typing import Any

from .route_verification_common import ROUTE_IDS
from .utils import dict_value, string_list


def judge_checklist(
    executed: list[dict[str, Any]],
    anchors: list[str],
    risks: list[dict[str, Any]],
    feedback_signals: list[dict[str, Any]],
    expectation: dict[str, Any],
) -> list[dict[str, Any]]:
    checks = [
        _evidence_grounding_check(anchors),
        _first_route_check(executed, expectation),
        _forbidden_route_check(executed, expectation),
        _risk_monitor_check(risks, expectation),
        _feedback_linkage_check(risks, feedback_signals),
    ]
    checks.extend(_conditional_checks(executed, risks, expectation))
    return checks


def _conditional_checks(
    executed: list[dict[str, Any]],
    risks: list[dict[str, Any]],
    expectation: dict[str, Any],
) -> list[dict[str, Any]]:
    checks: list[dict[str, Any]] = []
    if bool(expectation.get("requireExactCodeIdentity")):
        checks.append(_exact_code_identity_check(executed))
    if bool(expectation.get("requireNoExecutableLineRange")):
        checks.append(_line_range_check(risks))
    if bool(expectation.get("requireVerificationEvidence")):
        checks.append(_verification_evidence_check(executed))
    return checks


def _evidence_grounding_check(anchors: list[str]) -> dict[str, Any]:
    has_known_evidence = any(anchor != "unknown-workspace" for anchor in anchors)
    return _check(
        "evidence-state.grounded",
        "evidence-grounding",
        "pass" if has_known_evidence else "unknown",
        "Evidence state declares route-relevant anchors."
        if has_known_evidence
        else "Evidence state has no precise route anchor.",
        evidence_refs=[f"anchor:{anchor}" for anchor in anchors],
    )


def _first_route_check(
    executed: list[dict[str, Any]],
    expectation: dict[str, Any],
) -> dict[str, Any]:
    allowed = [route for route in string_list(expectation.get("allowedFirstRoutes"))]
    first_route = _first_route(executed)
    if first_route is None:
        return _check(
            "route.first.allowed",
            "route-choice",
            "unknown",
            "No executed route exists to judge first-route selection.",
        )
    if not allowed:
        return _check(
            "route.first.allowed",
            "route-choice",
            "unknown",
            "Scenario did not declare allowed first routes.",
            route=first_route,
            evidence_refs=_first_route_refs(executed),
        )
    return _check(
        "route.first.allowed",
        "route-choice",
        "pass" if first_route in allowed else "fail",
        f"First route {first_route} {'is' if first_route in allowed else 'is not'} allowed by evidence state.",
        route=first_route,
        evidence_refs=_first_route_refs(executed),
    )


def _forbidden_route_check(
    executed: list[dict[str, Any]],
    expectation: dict[str, Any],
) -> dict[str, Any]:
    forbidden = set(string_list(expectation.get("forbiddenRoutes")))
    observed = [route for route in _executed_routes(executed) if route in forbidden]
    return _check(
        "route.forbidden.avoided",
        "route-choice",
        "pass" if not observed else "fail",
        "Forbidden routes were avoided."
        if not observed
        else f"Forbidden routes were executed: {', '.join(observed)}.",
        evidence_refs=_route_refs(executed, observed),
    )


def _risk_monitor_check(
    risks: list[dict[str, Any]],
    expectation: dict[str, Any],
) -> dict[str, Any]:
    forbidden = set(string_list(expectation.get("forbiddenRiskFlags")))
    observed = [risk for risk in _risk_kinds(risks) if risk in forbidden]
    return _check(
        "risk.forbidden.absent",
        "risk-monitor",
        "pass" if not observed else "fail",
        "Forbidden risk flags were absent."
        if not observed
        else f"Forbidden risk flags were observed: {', '.join(observed)}.",
        evidence_refs=_risk_refs(risks, observed),
    )


def _exact_code_identity_check(executed: list[dict[str, Any]]) -> dict[str, Any]:
    exact_steps = [
        step
        for step in executed
        if step.get("route") == "query-code" and step.get("codePolicy") == "exact-only"
    ]
    return _check(
        "code.exact-identity",
        "code-policy",
        "pass" if exact_steps else "fail",
        "Exact query-code identity was recorded."
        if exact_steps
        else "Exact query-code identity was required but not recorded.",
        route="query-code",
        evidence_refs=_step_refs(exact_steps),
    )


def _line_range_check(risks: list[dict[str, Any]]) -> dict[str, Any]:
    matching = [risk for risk in risks if risk.get("kind") == "executable-line-range"]
    return _check(
        "selector.line-range-not-executable",
        "fallback-discipline",
        "pass" if not matching else "fail",
        "Line ranges stayed out of executable selector positions."
        if not matching
        else "A line range was used as an executable selector.",
        risk="executable-line-range",
        evidence_refs=_step_refs(matching),
    )


def _verification_evidence_check(executed: list[dict[str, Any]]) -> dict[str, Any]:
    refs = [ref for step in executed for ref in string_list(step.get("evidenceRefs"))]
    return _check(
        "verification.evidence-present",
        "verification-evidence",
        "pass" if refs else "fail",
        "Executed route steps carry replayable evidence refs."
        if refs
        else "Executed route steps do not carry replayable evidence refs.",
        evidence_refs=refs,
    )


def _feedback_linkage_check(
    risks: list[dict[str, Any]],
    feedback_signals: list[dict[str, Any]],
) -> dict[str, Any]:
    risk_kinds = [
        str(risk.get("kind"))
        for risk in risks
        if isinstance(risk.get("kind"), str)
    ]
    if not risk_kinds:
        return _check(
            "feedback.dataset-linked",
            "feedback-loop",
            "pass",
            "No route risks required feedback dataset linkage.",
        )

    signal_risks = {
        str(signal.get("riskKind"))
        for signal in feedback_signals
        if isinstance(signal.get("riskKind"), str)
    }
    unlinked_signals = [
        signal
        for signal in feedback_signals
        if not string_list(signal.get("userFeedbackRefs"))
    ]
    missing_risks = [risk for risk in risk_kinds if risk not in signal_risks]
    status = (
        "pass"
        if feedback_signals and not unlinked_signals and not missing_risks
        else "fail"
    )
    return _check(
        "feedback.dataset-linked",
        "feedback-loop",
        status,
        "Route risks are linked to feedback dataset entries."
        if status == "pass"
        else "Route risks are missing feedback dataset links.",
        evidence_refs=_feedback_refs(feedback_signals),
    )


def _check(
    check_id: str,
    kind: str,
    status: str,
    message: str,
    *,
    evidence_refs: list[str] | None = None,
    route: str | None = None,
    risk: str | None = None,
) -> dict[str, Any]:
    item: dict[str, Any] = {
        "id": check_id,
        "kind": kind,
        "status": status,
        "message": message,
    }
    if evidence_refs:
        item["evidenceRefs"] = evidence_refs
    if route in ROUTE_IDS:
        item["route"] = route
    if risk:
        item["risk"] = risk
    return item


def _first_route(executed: list[dict[str, Any]]) -> str | None:
    if not executed:
        return None
    value = executed[0].get("route")
    return str(value) if isinstance(value, str) else None


def _executed_routes(executed: list[dict[str, Any]]) -> list[str]:
    return [str(step.get("route")) for step in executed if isinstance(step, dict)]


def _risk_kinds(risks: list[dict[str, Any]]) -> list[str]:
    return [str(risk.get("kind")) for risk in risks if isinstance(risk, dict)]


def _first_route_refs(executed: list[dict[str, Any]]) -> list[str]:
    return _step_refs(executed[:1])


def _route_refs(executed: list[dict[str, Any]], routes: list[str]) -> list[str]:
    route_set = set(routes)
    return _step_refs([step for step in executed if step.get("route") in route_set])


def _risk_refs(risks: list[dict[str, Any]], risk_kinds: list[str]) -> list[str]:
    risk_set = set(risk_kinds)
    return _step_refs([risk for risk in risks if risk.get("kind") in risk_set])


def _step_refs(items: list[dict[str, Any]]) -> list[str]:
    refs: list[str] = []
    for item in items:
        refs.extend(string_list(item.get("evidenceRefs")))
        command_id = dict_value(item).get("commandId")
        if isinstance(command_id, str):
            refs.append(f"command:{command_id}")
    return list(dict.fromkeys(refs))


def _feedback_refs(signals: list[dict[str, Any]]) -> list[str]:
    refs: list[str] = []
    for signal in signals:
        refs.extend(string_list(signal.get("userFeedbackRefs")))
        refs.extend(string_list(signal.get("evidenceRefs")))
    return list(dict.fromkeys(refs))
