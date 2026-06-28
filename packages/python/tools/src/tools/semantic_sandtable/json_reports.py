"""JSON report rendering for sandtable results."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .models import ReceiptResult, ScenarioResult
from .report_format import scenario_totals
from .runtime_audit import runtime_audit_findings


def receipt_report_json(results: list[ReceiptResult]) -> dict[str, Any]:
    return {
        "summary": {
            "total": len(results),
            "pass": sum(1 for result in results if result.status == "pass"),
            "fail": sum(1 for result in results if result.status == "fail"),
        },
        "receipts": [
            {
                "path": str(result.path),
                "status": result.status,
                "scenarioId": result.scenario_id,
                "language": result.language,
                "commandCount": result.command_count,
                "stdoutBytes": result.stdout_bytes,
                "stderrBytes": result.stderr_bytes,
                "elapsedMs": result.elapsed_ms,
                "jsonSearches": result.json_searches,
                "compactSearches": result.compact_searches,
                "tokenCost": result.token_cost,
                "commandTokenCosts": result.command_token_costs,
                "querySetOpportunities": result.query_set_opportunities,
                "findings": result.findings,
                "errors": result.errors,
            }
            for result in results
        ],
    }


def report_json(
    results: list[ScenarioResult],
    repo_root: Path | None = None,
) -> dict[str, Any]:
    findings = runtime_audit_findings(results)
    return {
        "scenarios": [_scenario_json(result, repo_root) for result in results],
        "summary": {
            "total": len(results),
            "pass": sum(1 for result in results if result.status == "pass"),
            "warn": sum(1 for result in results if result.status == "warn"),
            "fail": sum(1 for result in results if result.status == "fail"),
            "skip": sum(1 for result in results if result.status == "skip"),
        },
        "audit": {
            "findings": [
                {
                    "kind": finding.kind,
                    "severity": finding.severity,
                    "scenarioId": finding.scenario_id,
                    "stepId": finding.step_id,
                    "message": _public_report_text(finding.message, repo_root),
                    "action": _public_report_text(finding.action, repo_root),
                }
                for finding in findings
            ],
            "summary": {
                "total": len(findings),
                "errors": sum(1 for finding in findings if finding.severity == "error"),
                "warnings": sum(
                    1 for finding in findings if finding.severity == "warning"
                ),
                "info": sum(1 for finding in findings if finding.severity == "info"),
            },
        },
    }


def _public_report_path(path: Path | None, repo_root: Path | None) -> str | None:
    if path is None:
        return None
    if repo_root is None:
        return str(path)
    try:
        return str(path.resolve().relative_to(repo_root.resolve()))
    except ValueError:
        return str(path).replace(str(repo_root.resolve()), "$ASP_REPO_ROOT")


def _public_report_text(text: str, repo_root: Path | None) -> str:
    if repo_root is None:
        return text
    return text.replace(str(repo_root.resolve()), "$ASP_REPO_ROOT")


def _scenario_json(
    result: ScenarioResult,
    repo_root: Path | None,
) -> dict[str, Any]:
    return {
        "id": result.scenario_id,
        "language": result.language,
        "path": _public_report_path(result.path, repo_root),
        "status": result.status,
        "workdir": _public_report_path(result.workdir, repo_root),
        "workdirSpec": result.workdir_spec,
        "coverage": result.coverage,
        "tags": result.tags,
        "evidence": result.evidence,
        "flowMetrics": scenario_totals(result) if result.evidence else {},
        "skipReason": result.skip_reason,
        "warnings": result.warnings,
        "errors": result.errors,
        "steps": [
            {
                "id": step.step_id,
                "command": step.command,
                "status": step.status,
                "exitCode": step.exit_code,
                "elapsedMs": step.elapsed_ms,
                "stdoutLines": step.stdout_lines,
                "stderrLines": step.stderr_lines,
                "stdoutBytes": step.stdout_bytes,
                "stderrBytes": step.stderr_bytes,
                "observations": step.observations,
                "warnings": step.warnings,
                "errors": step.errors,
            }
            for step in result.steps
        ],
    }
