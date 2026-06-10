"""Human-readable scenario report rendering."""

from __future__ import annotations

from pathlib import Path

from .models import ScenarioResult, StepResult
from .output import emit
from .report_format import quote_value, scenario_totals
from .runtime_audit import runtime_audit_findings
from .utils import dict_value, list_value, optional_int, require_str


def print_text_report(repo_root: Path, results: list[ScenarioResult]) -> None:
    summary = {"pass": 0, "warn": 0, "fail": 0, "skip": 0}
    for result in results:
        summary[result.status] = summary.get(result.status, 0) + 1
    emit(
        "[sandtable] "
        f"scenarios={len(results)} pass={summary['pass']} warn={summary['warn']} "
        f"fail={summary['fail']} skip={summary['skip']}"
    )
    for result in results:
        _print_scenario_report(repo_root, result)
    _print_runtime_audit(results)


def _print_scenario_report(repo_root: Path, result: ScenarioResult) -> None:
    emit(
        f"[scenario] id={result.scenario_id} lang={result.language} "
        f"status={result.status} path={_relative_path(repo_root, result.path)} "
        f"workdir={_relative_workdir(repo_root, result)}"
    )
    _print_scenario_metadata(result)
    _print_scenario_messages(result)
    for step in result.steps:
        _print_step_report(step)


def _relative_path(repo_root: Path, path: Path) -> Path:
    try:
        return path.relative_to(repo_root)
    except ValueError:
        return path


def _relative_workdir(repo_root: Path, result: ScenarioResult) -> str:
    if result.workdir is None:
        return "-"
    try:
        return str(result.workdir.relative_to(repo_root))
    except ValueError:
        return str(result.workdir)


def _print_scenario_metadata(result: ScenarioResult) -> None:
    if result.coverage:
        emit(f"|coverage {','.join(result.coverage)}")
    if result.tags:
        emit(f"|tags {','.join(result.tags)}")
    if result.evidence:
        _print_flow_report(result)
    if result.skip_reason:
        emit(f"|skip reason={quote_value(result.skip_reason)}")


def _print_scenario_messages(result: ScenarioResult) -> None:
    for warning in result.warnings:
        emit(f"|warn {warning}")
    for error in result.errors:
        emit(f"|error {error}")


def _print_step_report(step: StepResult) -> None:
    emit(
        f"|step {step.step_id} status={step.status} "
        f"exit={step.exit_code} ms={step.elapsed_ms} "
        f"stdout_lines={step.stdout_lines} stderr_lines={step.stderr_lines} "
        f"stdout_bytes={step.stdout_bytes}"
    )
    for warning in step.warnings:
        emit(f"|warn step={step.step_id} {warning}")
    for error in step.errors:
        emit(f"|error step={step.step_id} {error}")
    _print_step_observations(step)


def _print_step_observations(step: StepResult) -> None:
    token_cost = dict_value(step.observations.get("tokenCost"))
    if token_cost:
        emit(
            f"|tokens step={step.step_id} "
            f"input={optional_int(token_cost.get('inputTokens')) or 0} "
            f"output={optional_int(token_cost.get('outputTokens')) or 0} "
            f"cacheRead={optional_int(token_cost.get('cacheReadInputTokens')) or 0} "
            f"total={optional_int(token_cost.get('totalTokens')) or 0} "
            f"costUsd={quote_value(str(token_cost.get('costUsd', 'unknown')))}"
        )
    pipe_flow = dict_value(step.observations.get("pipeFlow"))
    if not pipe_flow:
        return
    missing = list_value(pipe_flow.get("missingComplexPipeStages"))
    emit(
        f"|pipeFlow step={step.step_id} "
        f"asp={optional_int(pipe_flow.get('aspCommands')) or 0} "
        f"search={optional_int(pipe_flow.get('searchCommands')) or 0} "
        f"query={optional_int(pipe_flow.get('queryCommands')) or 0} "
        f"directRead={optional_int(pipe_flow.get('directReadCommands')) or 0} "
        f"directReadBounded={optional_int(pipe_flow.get('directReadBoundedCommands')) or 0} "
        f"directReadRisk={optional_int(pipe_flow.get('directReadRiskCommands')) or 0} "
        f"repeated={optional_int(pipe_flow.get('repeatedCommands')) or 0} "
        f"complex={str(bool(pipe_flow.get('complexPipeFlow'))).lower()} "
        f"missing={quote_value(','.join(str(item) for item in missing) or '-')}"
    )
    _print_pipe_flow_commands(step.step_id, pipe_flow)
    _print_pipe_flow_outputs(step.step_id, pipe_flow)


def _print_pipe_flow_commands(step_id: str, pipe_flow: dict[str, object]) -> None:
    commands = [
        str(command)
        for command in list_value(pipe_flow.get("commands"))
        if isinstance(command, str)
    ]
    if not commands:
        return
    parts = [
        f"C{index}={quote_value(_compact_command(command))}"
        for index, command in enumerate(commands[:8], start=1)
    ]
    emit(f"|pipeCommands step={step_id} {' '.join(parts)}")


def _print_pipe_flow_outputs(step_id: str, pipe_flow: dict[str, object]) -> None:
    records = [
        record
        for record in list_value(pipe_flow.get("aspCommandOutputRecords"))
        if isinstance(record, dict)
    ]
    if not records:
        return
    for index, record in enumerate(records[:4], start=1):
        command = require_str(record, "command", "-")
        parts = [
            f"R{index}",
            f"bytes={optional_int(record.get('outputBytes')) or 0}",
            f"lines={optional_int(record.get('outputLines')) or 0}",
            f"denied={str(bool(record.get('denied'))).lower()}",
            f"cmd={quote_value(_compact_command(command))}",
        ]
        fingerprint = record.get("outputFingerprint")
        if isinstance(fingerprint, str):
            parts.append(f"fp={quote_value(fingerprint)}")
        feedback = record.get("hookFeedback")
        if isinstance(feedback, str):
            parts.append(f"hookFeedback={quote_value(feedback)}")
        preview = record.get("outputPreview")
        if isinstance(preview, str):
            parts.append(f"preview={quote_value(preview)}")
        emit(f"|pipeOutput step={step_id} {' '.join(parts)}")


def _compact_command(command: str) -> str:
    command = " ".join(command.split())
    return command if len(command) <= 220 else f"{command[:217]}..."


def _print_runtime_audit(results: list[ScenarioResult]) -> None:
    findings = runtime_audit_findings(results)
    if not findings:
        return
    error_count = sum(1 for finding in findings if finding.severity == "error")
    warning_count = sum(1 for finding in findings if finding.severity == "warning")
    info_count = sum(1 for finding in findings if finding.severity == "info")
    emit(
        "[sandtable-audit] "
        f"findings={len(findings)} errors={error_count} "
        f"warnings={warning_count} info={info_count}"
    )
    for finding in findings:
        step = "" if finding.step_id is None else f" step={finding.step_id}"
        emit(
            f"|audit kind={finding.kind} severity={finding.severity} "
            f"scenario={finding.scenario_id}{step} "
            f"message={quote_value(finding.message)} "
            f"action={quote_value(finding.action)}"
        )


def _print_flow_report(result: ScenarioResult) -> None:
    totals = scenario_totals(result)
    source = require_str(result.evidence, "source", "unknown")
    edit_boundary = require_str(result.evidence, "editBoundary", "unknown")
    run_count = _scenario_run_count(result, totals)
    emit(
        "[sandtable-flow] "
        f"scenario={result.scenario_id} source={source} "
        f"{run_count} stdoutBytes={totals['stdoutBytes']} "
        f"stderrBytes={totals['stderrBytes']} elapsedMs={totals['elapsedMs']} "
        f"editBoundary={edit_boundary}"
    )
    intent = result.evidence.get("intent")
    if isinstance(intent, str):
        emit(f"|intent {quote_value(intent)}")
    metrics = dict_value(result.evidence.get("metrics"))
    if metrics:
        parts = [
            f"{key}={quote_value(str(value))}"
            for key, value in sorted(metrics.items())
            if isinstance(value, (str, int, float, bool))
        ]
        if parts:
            emit(f"|recorded {' '.join(parts)}")
    _print_failure_frontier_comparison(result)
    for opportunity in list_value(result.evidence.get("querySetOpportunities")):
        if not isinstance(opportunity, dict):
            continue
        view = require_str(opportunity, "view", "unknown")
        queries = optional_int(opportunity.get("queries")) or 0
        save_commands = optional_int(opportunity.get("saveCommands")) or 0
        selector = require_str(opportunity, "selector", "-")
        reason = opportunity.get("reason")
        line = (
            f"|merge view={view} queries={queries} "
            f"saveCommands={save_commands} selector={selector}"
        )
        if isinstance(reason, str):
            line = f"{line} reason={quote_value(reason)}"
        emit(line)
    for finding in list_value(result.evidence.get("findings")):
        if not isinstance(finding, dict):
            continue
        kind = require_str(finding, "kind", "unknown")
        severity = require_str(finding, "severity", "info")
        message = require_str(finding, "message", "")
        emit(f"|finding kind={kind} severity={severity} message={quote_value(message)}")


def _scenario_run_count(result: ScenarioResult, totals: dict[str, int]) -> str:
    label = "prompts" if "prompt-only" in result.tags else "commands"
    return f"{label}={totals['commands']}"


def _print_failure_frontier_comparison(result: ScenarioResult) -> None:
    comparison = dict_value(result.evidence.get("failureFrontierComparisonResult"))
    if not comparison:
        return
    baseline = dict_value(comparison.get("baseline"))
    candidate = dict_value(comparison.get("candidate"))
    delta = dict_value(comparison.get("delta"))
    frontier = dict_value(comparison.get("frontier"))
    emit(
        "|failureFrontier "
        f"status={require_str(comparison, 'status', 'unknown')} "
        f"baselineCommands={optional_int(baseline.get('commandCount')) or 0} "
        f"candidateCommands={optional_int(candidate.get('commandCount')) or 0} "
        "commandReductionRatio="
        f"{float(delta.get('commandReductionRatio') or 0):.3f} "
        f"directSourceReadCode={candidate.get('directSourceReadCodeCount', 0)} "
        f"duplicateSelectors={candidate.get('duplicateSelectorCount', 0)} "
        f"sameFileWindowFanout={candidate.get('sameFileWindowFanout', 0)} "
        f"missingHotBlocks={len(list_value(frontier.get('missingHotBlocks')))}"
    )
