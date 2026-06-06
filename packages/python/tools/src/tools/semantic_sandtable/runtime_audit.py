"""Runtime audit synthesis for executed sandtable results."""

from __future__ import annotations

from .models import RuntimeAuditFinding, ScenarioResult, StepResult
from .report_format import scenario_totals
from .utils import dict_value, string_list


def runtime_audit_findings(results: list[ScenarioResult]) -> list[RuntimeAuditFinding]:
    findings: list[RuntimeAuditFinding] = []
    for result in results:
        findings.extend(_scenario_status_findings(result))
        findings.extend(_step_status_findings(result))
    findings.extend(_large_library_runtime_findings(results))
    findings.extend(_top_cost_findings(results))
    return findings


def _scenario_status_findings(result: ScenarioResult) -> list[RuntimeAuditFinding]:
    findings: list[RuntimeAuditFinding] = []
    if result.status == "fail":
        message = "; ".join(result.errors) if result.errors else "scenario failed"
        findings.append(
            RuntimeAuditFinding(
                kind="scenario-failure",
                severity="error",
                scenario_id=result.scenario_id,
                message=message,
                action="fix the failing scenario contract or provider command before tuning budgets",
            )
        )
    for warning in result.warnings:
        findings.append(
            RuntimeAuditFinding(
                kind="scenario-budget",
                severity="warning",
                scenario_id=result.scenario_id,
                message=warning,
                action="reduce packet size/latency or adjust the scenario budget with fresh evidence",
            )
        )
    return findings


def _step_status_findings(result: ScenarioResult) -> list[RuntimeAuditFinding]:
    findings: list[RuntimeAuditFinding] = []
    for step in result.steps:
        findings.extend(_step_error_findings(result, step))
        findings.extend(_step_warning_findings(result, step))
    return findings


def _step_error_findings(
    result: ScenarioResult,
    step: StepResult,
) -> list[RuntimeAuditFinding]:
    if not step.errors:
        return []
    return [
        RuntimeAuditFinding(
            kind="step-failure",
            severity="error",
            scenario_id=result.scenario_id,
            step_id=step.step_id,
            message=error,
            action="inspect the step command, stdin capture, and expected protocol lines",
        )
        for error in step.errors[:3]
    ]


def _step_warning_findings(
    result: ScenarioResult,
    step: StepResult,
) -> list[RuntimeAuditFinding]:
    return [
        RuntimeAuditFinding(
            kind=_warning_kind(warning),
            severity="warning",
            scenario_id=result.scenario_id,
            step_id=step.step_id,
            message=warning,
            action=_warning_action(warning),
        )
        for warning in step.warnings[:3]
    ]


def _warning_kind(warning: str) -> str:
    if "stdoutBytes" in warning or "stdoutLines" in warning:
        return "packet-size-budget"
    if "elapsedMs" in warning:
        return "latency-budget"
    return "step-budget"


def _warning_action(warning: str) -> str:
    if "stdoutBytes" in warning or "stdoutLines" in warning:
        return "prefer compact views, stronger caps, or query-set synthesis before widening output"
    if "elapsedMs" in warning:
        return "profile provider startup/indexing or raise the threshold only with repeated evidence"
    return "compare expected budget with current runtime evidence"


def _large_library_runtime_findings(
    results: list[ScenarioResult],
) -> list[RuntimeAuditFinding]:
    findings: list[RuntimeAuditFinding] = []
    for result in results:
        if "large-library" not in result.coverage or result.status != "skip":
            continue
        findings.append(
            RuntimeAuditFinding(
                kind="large-library-skip",
                severity="info",
                scenario_id=result.scenario_id,
                message=_large_library_skip_message(result),
                action=_large_library_skip_action(result),
            )
        )
    return findings


def _large_library_skip_message(result: ScenarioResult) -> str:
    target = dict_value(result.evidence.get("targetLibrary"))
    package = target.get("package")
    reason = result.skip_reason or "large-library checkout unavailable"
    return f"package={package} reason={reason}" if isinstance(package, str) else reason


def _large_library_skip_action(result: ScenarioResult) -> str:
    workdir_spec = dict_value(result.workdir_spec)
    env_name = workdir_spec.get("env")
    candidates = string_list(workdir_spec.get("candidates"))
    if isinstance(env_name, str) and candidates:
        return (
            f"set {env_name} or add one checkout matching "
            f"{','.join(candidates)} to collect runtime evidence"
        )
    if isinstance(env_name, str):
        return f"set {env_name} to collect runtime evidence"
    if candidates:
        return (
            "add one checkout matching "
            f"{','.join(candidates)} to collect runtime evidence"
        )
    return "set the scenario workdir env var or add a checkout candidate to collect runtime evidence"


def _top_cost_findings(results: list[ScenarioResult]) -> list[RuntimeAuditFinding]:
    executed = [
        result
        for result in results
        if result.steps and result.status in {"pass", "warn", "fail"}
    ]
    if not executed:
        return []
    by_stdout = max(executed, key=lambda result: scenario_totals(result)["stdoutBytes"])
    by_elapsed = max(executed, key=lambda result: scenario_totals(result)["elapsedMs"])
    findings = [_top_stdout_finding(by_stdout)]
    if by_elapsed.scenario_id != by_stdout.scenario_id:
        findings.append(_top_elapsed_finding(by_elapsed))
    return findings


def _top_stdout_finding(result: ScenarioResult) -> RuntimeAuditFinding:
    totals = scenario_totals(result)
    return RuntimeAuditFinding(
        kind="top-stdout-cost",
        severity="info",
        scenario_id=result.scenario_id,
        message=f"stdoutBytes={totals['stdoutBytes']} commands={totals['commands']}",
        action="inspect whether large packets need tighter seeds, compact view, or query-set compression",
    )


def _top_elapsed_finding(result: ScenarioResult) -> RuntimeAuditFinding:
    totals = scenario_totals(result)
    return RuntimeAuditFinding(
        kind="top-latency-cost",
        severity="info",
        scenario_id=result.scenario_id,
        message=f"elapsedMs={totals['elapsedMs']} commands={totals['commands']}",
        action="check provider startup/indexing cost before widening this scenario",
    )
