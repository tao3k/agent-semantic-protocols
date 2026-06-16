"""Run simulated deep-search sessions from adaptive validation manifests."""

from __future__ import annotations

import json
import shlex
import shutil
import subprocess
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

from .agent_session import (
    write_agent_session_from_messages,
    write_agent_session_receipt,
)
from .agent_session_analyzer import write_agent_session_analysis
from .agent_session_question_plan import write_aggregated_agent_session_question_plan
from .large_library_adaptive_signals import (
    _command_was_run,
    _empty_step_signals,
    _final_step_signals,
    _line_value,
    _recovery_probe_signals,
    _should_follow_after_third,
    _should_probe_after_final,
    _third_step_signals,
)
from .large_library_adaptive_report import (
    _algorithm_improvement_plan,
    _owner_items_recovery_cases,
    _selector_quality_cases,
    _summary,
    _variant_summaries,
)
from .large_library_adaptive_session import _messages_for_results, _session_config
from .large_library_adaptive_validation import (
    build_large_library_adaptive_validation_report,
)
from .output import write_json_file
from .utils import build_env, dict_value, list_value, require_str, resolve_path, resolve_workdir

CommandRunner = Callable[[list[str], Path, dict[str, str]], dict[str, Any]]

_SCHEMA_ID = "agent.semantic-protocols.semantic-graph-turbo-adaptive-simulation-report"


def run_large_library_adaptive_simulation(
    repo_root: Path,
    policy: dict[str, Any],
    manifest: dict[str, Any],
    output_root: Path,
    *,
    runner: CommandRunner | None = None,
    limit: int | None = None,
) -> dict[str, Any]:
    output_root.mkdir(parents=True, exist_ok=True)
    command_runner = runner or _run_command
    run_reports = [
        _simulate_run(repo_root, run, output_root, command_runner)
        for run in _selected_runs(manifest, limit)
    ]
    question_plan_paths = [
        Path(report["questionImprovementPlanPath"])
        for report in run_reports
        if report["status"] == "pass"
    ]
    question_plan_path = output_root / "question-plan-aggregate.json"
    question_plan = write_aggregated_agent_session_question_plan(
        question_plan_paths,
        question_plan_path,
        session_id="adaptive-simulation-question-plan-aggregate",
        scenario_id="adaptive-simulation.question-plan-aggregate",
    )
    validation = build_large_library_adaptive_validation_report(policy, question_plan)
    validation_path = output_root / "adaptive-validation-report.json"
    write_json_file(validation_path, validation)
    report = {
        "schemaId": _SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-adaptive-simulation-report",
        "targetGraphPhase": "query-first-stage",
        "summary": _summary(run_reports),
        "variantSummaries": _variant_summaries(run_reports),
        "algorithmImprovementPlan": _algorithm_improvement_plan(run_reports),
        "ownerItemsRecoveryCases": _owner_items_recovery_cases(run_reports),
        "selectorQualityCases": _selector_quality_cases(run_reports),
        "questionPlanAggregatePath": str(question_plan_path),
        "validationReportPath": str(validation_path),
        "runReports": run_reports,
    }
    write_json_file(output_root / "adaptive-simulation-report.json", report)
    return report


def _selected_runs(manifest: dict[str, Any], limit: int | None) -> list[dict[str, Any]]:
    runs = [item for item in list_value(manifest.get("runs")) if isinstance(item, dict)]
    return runs if limit is None else runs[:limit]


def _simulate_run(
    repo_root: Path,
    run: dict[str, Any],
    output_root: Path,
    runner: CommandRunner,
) -> dict[str, Any]:
    run_id = require_str(run, "runId", "unknown")
    session_root = output_root / "sessions" / _safe_id(run_id)
    source_workdir = _workdir_for_run(repo_root, run)
    if source_workdir is None:
        return _skipped_run(run, "workdir-not-found", session_root)
    workdir = _activated_workdir(source_workdir, output_root, run_id)
    asp_bin = _asp_bin(repo_root)
    commands = _commands_for_run(run, workdir, asp_bin)
    env = _command_env(run, repo_root, output_root)
    results = _run_search_commands(commands, workdir, env, runner, asp_bin)
    config = _session_config(run, workdir)
    messages = _messages_for_results(run, results)
    write_agent_session_from_messages(messages, session_root, config=config)
    receipt_path = session_root / "receipts" / "agent-session-receipt.json"
    write_agent_session_receipt(session_root, receipt_path, config=config)
    report_path = session_root / "reports" / "quality-report.json"
    feedback_path = session_root / "reports" / "graph-turbo-feedback.json"
    improvement_path = session_root / "reports" / "improvement-report.json"
    question_plan_path = session_root / "reports" / "question-improvement-plan.json"
    write_agent_session_analysis(
        receipt_path,
        report_path,
        feedback_path,
        improvement_path,
        question_plan_path=question_plan_path,
    )
    return {
        "runId": run_id,
        "scenarioId": require_str(run, "scenarioId", "unknown"),
        "questionId": require_str(run, "questionId", "unknown"),
        "ablationVariant": require_str(run, "ablationVariant", "unknown"),
        "language": require_str(run, "language", "unknown"),
        "project": dict_value(run.get("project")),
        "status": "pass" if all(result["exitCode"] == 0 for result in results) else "fail",
        "workdir": str(workdir),
        "commandCount": len(results),
        "elapsedMs": sum(int(result["elapsedMs"]) for result in results),
        "stdoutBytes": sum(int(result["stdoutBytes"]) for result in results),
        "pipeSignals": _pipe_signals(results[1]["stdout"] if len(results) > 1 else ""),
        "thirdStepSignals": _third_step_signals(results),
        "finalStepSignals": _final_step_signals(results),
        "recoveryProbeSignals": _recovery_probe_signals(results),
        "receiptPath": str(receipt_path),
        "qualityReportPath": str(report_path),
        "graphTurboFeedbackPath": str(feedback_path),
        "improvementReportPath": str(improvement_path),
        "questionImprovementPlanPath": str(question_plan_path),
        "commands": results,
    }


def _run_search_commands(
    commands: list[list[str]],
    workdir: Path,
    env: dict[str, str],
    runner: CommandRunner,
    asp_bin: str,
) -> list[dict[str, Any]]:
    results = [_run_timed(command, workdir, env, runner) for command in commands[:2]]
    next_command = (
        _next_command(results[-1]["stdout"], asp_bin) if len(results) >= 2 else None
    )
    if not next_command:
        return results
    results.append(_run_timed(next_command, workdir, env, runner))
    third_signals = _third_step_signals(results)
    followup_command = _successful_next_command(results[-1], asp_bin)
    if (
        followup_command
        and _should_follow_after_third(third_signals)
        and not _command_was_run(followup_command, results)
    ):
        results.append(_run_timed(followup_command, workdir, env, runner))
        final_signals = _final_step_signals(results)
        probe_command = _successful_next_command(results[-1], asp_bin)
        if (
            probe_command
            and _should_probe_after_final(final_signals)
            and not _command_was_run(probe_command, results)
        ):
            results.append(_run_timed(probe_command, workdir, env, runner))
    return results


def _successful_next_command(result: dict[str, Any], asp_bin: str) -> list[str] | None:
    if int(result["exitCode"]) != 0:
        return None
    return _next_command(str(result["stdout"]), asp_bin)


def _workdir_for_run(repo_root: Path, run: dict[str, Any]) -> Path | None:
    scenario_path = resolve_path(repo_root, require_str(run, "scenarioPath", ""))
    if scenario_path is None or not scenario_path.is_file():
        return None
    scenario = json.loads(scenario_path.read_text(encoding="utf-8"))
    return resolve_workdir(repo_root, dict_value(scenario).get("workdir"))


def _commands_for_run(run: dict[str, Any], workdir: Path, asp_bin: str) -> list[list[str]]:
    language = require_str(run, "language", "unknown")
    prompt = require_str(run, "prompt", require_str(run, "questionId", ""))
    return [
        [
            asp_bin,
            language,
            "search",
            "prime",
            "--workspace",
            str(workdir),
            "--view",
            "seeds",
        ],
        [
            asp_bin,
            language,
            "search",
            "pipe",
            prompt,
            "--workspace",
            str(workdir),
            "--view",
            "seeds",
        ],
    ]


def _activated_workdir(source: Path, output_root: Path, run_id: str) -> Path:
    if _has_git_toplevel(source):
        return source
    target = output_root / "workdirs" / _safe_id(run_id)
    if not target.exists():
        shutil.copytree(source, target, ignore=_copy_ignore(source, output_root))
    if not (target / ".git").exists():
        subprocess.run(["git", "init", "-q"], cwd=target, check=True)
    return target


def _copy_ignore(source: Path, output_root: Path) -> Callable[[str, list[str]], set[str]]:
    ignored = {".git", "target"}
    try:
        ignored.add(output_root.relative_to(source).parts[0])
    except ValueError:
        pass
    return lambda _directory, names: {name for name in names if name in ignored}


def _has_git_toplevel(path: Path) -> bool:
    return (
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=path,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode
        == 0
    )


def _command_env(
    run: dict[str, Any],
    repo_root: Path,
    output_root: Path,
) -> dict[str, str]:
    env = build_env(run.get("env"), repo_root=repo_root)
    for key in (
        "PRJ_ROOT",
        "PRJ_CONFIG_HOME",
        "PRJ_DATA_HOME",
        "PRJ_PATH",
        "PRJ_RUNTIME_DIR",
    ):
        env.pop(key, None)
    env["PRJ_CACHE_HOME"] = str(output_root / "prj-cache")
    return env


def _run_timed(
    command: list[str],
    workdir: Path,
    env: dict[str, str],
    runner: CommandRunner,
) -> dict[str, Any]:
    started = time.monotonic()
    result = runner(command, workdir, env)
    elapsed_ms = int((time.monotonic() - started) * 1000)
    stdout = str(result.get("stdout", ""))
    stderr = str(result.get("stderr", ""))
    return {
        "command": shlex.join(command),
        "argv": command,
        "exitCode": int(result.get("exitCode", 0)),
        "elapsedMs": elapsed_ms,
        "stdout": stdout,
        "stderr": stderr,
        "stdoutBytes": len(stdout.encode("utf-8")),
        "stderrBytes": len(stderr.encode("utf-8")),
    }


def _run_command(command: list[str], workdir: Path, env: dict[str, str]) -> dict[str, Any]:
    completed = subprocess.run(
        command,
        cwd=workdir,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=90,
        check=False,
    )
    return {
        "exitCode": completed.returncode,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
    }


def _next_command(stdout: str, asp_bin: str) -> list[str] | None:
    for line in stdout.splitlines():
        if line.startswith("nextCommand="):
            command = line.split("=", 1)[1].strip()
            if command.startswith("asp "):
                argv = shlex.split(command)
                return [asp_bin, *argv[1:]]
    return None


def _pipe_signals(stdout: str) -> dict[str, Any]:
    return {
        "queryQuality": _line_value(stdout, "queryQuality"),
        "packageCohesion": _line_value(stdout, "packageCohesion"),
        "risk": _line_value(stdout, "risk"),
        "recommendedNext": _line_value(stdout, "recommendedNext"),
        "nextCommandPresent": bool(_next_command(stdout, "asp")),
    }


def _skipped_run(run: dict[str, Any], reason: str, session_root: Path) -> dict[str, Any]:
    return {
        "runId": require_str(run, "runId", "unknown"),
        "scenarioId": require_str(run, "scenarioId", "unknown"),
        "questionId": require_str(run, "questionId", "unknown"),
        "ablationVariant": require_str(run, "ablationVariant", "unknown"),
        "language": require_str(run, "language", "unknown"),
        "project": dict_value(run.get("project")),
        "status": "skipped",
        "skipReason": reason,
        "sessionRoot": str(session_root),
        "commandCount": 0,
        "elapsedMs": 0,
        "stdoutBytes": 0,
        "pipeSignals": {},
        "thirdStepSignals": _empty_step_signals(),
        "finalStepSignals": _empty_step_signals(),
        "recoveryProbeSignals": _empty_step_signals(),
        "commands": [],
    }


def _asp_bin(repo_root: Path) -> str:
    for relative in ("target/debug/asp", ".bin/asp"):
        candidate = repo_root / relative
        if candidate.exists():
            return str(candidate)
    return "asp"


def _safe_id(value: str) -> str:
    return "".join(c if c.isalnum() or c in {".", "-", "_"} else "-" for c in value).strip("-") or "session"
