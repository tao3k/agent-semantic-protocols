"""Runtime audit synthesis for executed sandtable results."""

from __future__ import annotations

from .models import RuntimeAuditFinding, ScenarioResult, StepResult
from .report_format import scenario_totals
from .utils import dict_value, optional_float, optional_int, string_list


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
    git_spec = dict_value(workdir_spec.get("git"))
    if git_spec:
        cache_key = git_spec.get("cacheKey")
        cache_hint = (
            f".cache/sandtable-repos/{cache_key}"
            if isinstance(cache_key, str)
            else ".cache/sandtable-repos/<cacheKey>"
        )
        return (
            "set the missing live-run environment flags and use cached git "
            f"workdir {cache_hint}; delete that checkout only when the pinned ref changes"
        )
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
    if asp_output_findings := _top_asp_command_output_findings(executed):
        findings.extend(asp_output_findings)
    if token_findings := _top_agent_token_findings(executed):
        findings.extend(token_findings)
    if by_elapsed.scenario_id != by_stdout.scenario_id:
        findings.append(_top_elapsed_finding(by_elapsed))
    return findings


def _top_asp_command_output_findings(
    results: list[ScenarioResult],
) -> list[RuntimeAuditFinding]:
    measured = [
        (result, _scenario_asp_command_output_bytes(result))
        for result in results
    ]
    measured = [(result, bytes_) for result, bytes_ in measured if bytes_ > 0]
    if not measured:
        return []
    result, bytes_ = max(measured, key=lambda item: item[1])
    totals = scenario_totals(result)
    return [
        RuntimeAuditFinding(
            kind="top-asp-command-output-cost",
            severity="info",
            scenario_id=result.scenario_id,
            message=f"aspCommandOutputBytes={bytes_} commands={totals['commands']}",
            action=(
                "tighten ASP pipe/query projections before reducing parser-owned "
                "semantic facts"
            ),
        )
    ]


def _top_agent_token_findings(
    results: list[ScenarioResult],
) -> list[RuntimeAuditFinding]:
    measured = [
        (result, _scenario_agent_token_cost(result))
        for result in results
    ]
    measured = [
        (result, token_cost)
        for result, token_cost in measured
        if optional_int(token_cost.get("totalTokens")) is not None
    ]
    if not measured:
        return []
    result, token_cost = max(
        measured,
        key=lambda item: optional_int(item[1].get("totalTokens")) or 0,
    )
    totals = scenario_totals(result)
    return [
        RuntimeAuditFinding(
            kind="top-agent-token-cost",
            severity="info",
            scenario_id=result.scenario_id,
            message=_agent_token_cost_message(token_cost, totals["commands"]),
            action=(
                "optimize prompt, runtime settings, and selected context after "
                "preserving required ASP semantic facts"
            ),
        )
    ]


def _top_stdout_finding(result: ScenarioResult) -> RuntimeAuditFinding:
    totals = scenario_totals(result)
    return RuntimeAuditFinding(
        kind="top-stdout-cost",
        severity="info",
        scenario_id=result.scenario_id,
        message=f"stdoutBytes={totals['stdoutBytes']} commands={totals['commands']}",
        action="inspect runner stdout separately from agent-visible ASP output and SDK token usage",
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


def _scenario_asp_command_output_bytes(result: ScenarioResult) -> int:
    total = 0
    for step in result.steps:
        pipe_flow = dict_value(step.observations.get("pipeFlow"))
        total += optional_int(pipe_flow.get("aspCommandOutputBytes")) or 0
    return total


def _scenario_agent_token_cost(result: ScenarioResult) -> dict[str, int | float]:
    int_fields = (
        "inputTokens",
        "outputTokens",
        "cacheCreationInputTokens",
        "cacheReadInputTokens",
        "cacheWriteInputTokens",
        "totalTokens",
        "usageRecords",
    )
    totals = {field: 0 for field in int_fields}
    cost_usd = 0.0
    saw_cost = False
    for step in result.steps:
        token_cost = dict_value(step.observations.get("tokenCost"))
        for field in int_fields:
            totals[field] += optional_int(token_cost.get(field)) or 0
        cost = optional_float(token_cost.get("costUsd"))
        if cost is not None:
            cost_usd += cost
            saw_cost = True
    compact: dict[str, int | float] = {
        field: total for field, total in totals.items() if total
    }
    if "totalTokens" not in compact:
        total_tokens = sum(
            totals[field]
            for field in int_fields
            if field not in {"totalTokens", "usageRecords"}
        )
        if total_tokens:
            compact["totalTokens"] = total_tokens
    if saw_cost:
        compact["costUsd"] = cost_usd
    return compact


def _agent_token_cost_message(token_cost: dict[str, int | float], commands: int) -> str:
    parts = [
        f"totalTokens={optional_int(token_cost.get('totalTokens')) or 0}",
        f"inputTokens={optional_int(token_cost.get('inputTokens')) or 0}",
        f"outputTokens={optional_int(token_cost.get('outputTokens')) or 0}",
        f"cacheReadInputTokens={optional_int(token_cost.get('cacheReadInputTokens')) or 0}",
    ]
    cost_usd = optional_float(token_cost.get("costUsd"))
    if cost_usd is not None:
        parts.append(f"costUsd={cost_usd:.6f}")
    parts.append(f"commands={commands}")
    return " ".join(parts)
