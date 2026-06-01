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
    emit(
        "[sandtable-flow] "
        f"scenario={result.scenario_id} source={source} "
        f"commands={totals['commands']} stdoutBytes={totals['stdoutBytes']} "
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
