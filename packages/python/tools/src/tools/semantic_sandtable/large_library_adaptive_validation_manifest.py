"""Build executable live-agent validation manifests from adaptive policies."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .utils import dict_value, list_value, optional_int, require_str, resolve_path

_SCHEMA_ID = "agent.semantic-protocols.semantic-graph-turbo-adaptive-validation-manifest"


def build_large_library_adaptive_validation_manifest(
    repo_root: Path,
    policy: dict[str, Any],
    *,
    session_root: str = ".cache/agent-semantic-protocol/adaptive-validation",
) -> dict[str, Any]:
    runs = [
        _manifest_run(repo_root, run, session_root)
        for run in _policy_runs(policy)
    ]
    return {
        "schemaId": _SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-adaptive-validation-manifest",
        "targetGraphPhase": "query-first-stage",
        "sourcePolicy": _source_policy(policy),
        "summary": _summary(runs),
        "runs": runs,
    }


def _policy_runs(policy: dict[str, Any]) -> list[dict[str, Any]]:
    validation_plan = dict_value(policy.get("validationPlan"))
    return [
        item
        for item in list_value(validation_plan.get("runs"))
        if isinstance(item, dict)
    ]


def _manifest_run(
    repo_root: Path,
    run: dict[str, Any],
    session_root: str,
) -> dict[str, Any]:
    scenario_path = require_str(run, "scenarioPath", "unknown")
    scenario = _load_scenario(repo_root, scenario_path)
    question_id = require_str(run, "questionId", "unknown")
    question_case = _question_case(scenario, question_id)
    prompt = require_str(question_case, "question", "")
    target_library = dict_value(dict_value(scenario.get("evidence")).get("targetLibrary"))
    language = require_str(scenario, "language", require_str(target_library, "language", "unknown"))
    project_name = require_str(
        target_library,
        "package",
        require_str(target_library, "name", "unknown"),
    )
    project_source = require_str(target_library, "workdirKind", "registry")
    run_id = require_str(run, "runId", "unknown")
    run_root = f"{session_root.rstrip('/')}/{_safe_id(run_id)}"
    command_args = _record_command_args(
        run,
        prompt=prompt or question_id,
        language=language,
        project_name=project_name,
        project_source=project_source,
        session_root=run_root,
        max_asp_commands=optional_int(dict_value(question_case.get("audit")).get("maxAspCommands")),
    )
    return {
        "runId": run_id,
        "scenarioId": require_str(run, "scenarioId", "unknown"),
        "scenarioPath": scenario_path,
        "questionId": question_id,
        "promptResolved": bool(prompt),
        "prompt": prompt,
        "language": language,
        "project": {
            "name": project_name,
            "source": project_source,
            "targetLibrary": target_library,
        },
        "ablationVariant": require_str(run, "ablationVariant", "unknown"),
        "env": dict_value(run.get("env")),
        "sessionRoot": run_root,
        "commandArgs": command_args,
        "expectedArtifacts": {
            "receiptPath": f"{run_root}/receipts/agent-session-receipt.json",
            "qualityReportPath": f"{run_root}/reports/quality-report.json",
            "graphTurboFeedbackPath": f"{run_root}/reports/graph-turbo-feedback.json",
            "improvementReportPath": f"{run_root}/reports/improvement-report.json",
            "questionImprovementPlanPath": (
                f"{run_root}/reports/question-improvement-plan.json"
            ),
        },
    }


def _load_scenario(repo_root: Path, scenario_path: str) -> dict[str, Any]:
    resolved = resolve_path(repo_root, scenario_path)
    if resolved is None or not resolved.is_file():
        return {}
    value = json.loads(resolved.read_text(encoding="utf-8"))
    return value if isinstance(value, dict) else {}


def _question_case(scenario: dict[str, Any], question_id: str) -> dict[str, Any]:
    evidence = dict_value(scenario.get("evidence"))
    for item in list_value(evidence.get("deepQuestionCases")):
        if isinstance(item, dict) and item.get("id") == question_id:
            return item
    return {}


def _record_command_args(
    run: dict[str, Any],
    *,
    prompt: str,
    language: str,
    project_name: str,
    project_source: str,
    session_root: str,
    max_asp_commands: int | None,
) -> list[str]:
    run_id = require_str(run, "runId", "unknown")
    args = [
        "--record-agent-session",
        "--analyzer",
        "--agent-session-root",
        session_root,
        "--session-id",
        run_id,
        "--scenario-id",
        require_str(run, "scenarioId", "unknown"),
        "--language",
        language,
        "--project-name",
        project_name,
        "--project-source",
        project_source,
        "--intent",
        prompt,
        "--prompt",
        prompt,
        "--include-hook-events",
        "--require-asp-bash-commands",
    ]
    if max_asp_commands is not None:
        args.extend(["--max-asp-bash-commands", str(max_asp_commands)])
    return args


def _source_policy(policy: dict[str, Any]) -> dict[str, Any]:
    validation_plan = dict_value(policy.get("validationPlan"))
    default_policy = dict_value(policy.get("defaultPolicy"))
    return {
        "schemaId": policy.get("schemaId"),
        "packetKind": policy.get("packetKind"),
        "status": policy.get("status"),
        "defaultVariant": default_policy.get("ablationVariant"),
        "plannedRunCount": validation_plan.get("runCount"),
    }


def _summary(runs: list[dict[str, Any]]) -> dict[str, Any]:
    missing_prompt_count = sum(1 for run in runs if not run["promptResolved"])
    variant_counts: dict[str, int] = {}
    language_counts: dict[str, int] = {}
    project_counts: dict[str, int] = {}
    for run in runs:
        _increment(variant_counts, require_str(run, "ablationVariant", "unknown"))
        _increment(language_counts, require_str(run, "language", "unknown"))
        project = dict_value(run.get("project"))
        _increment(project_counts, require_str(project, "name", "unknown"))
    return {
        "runCount": len(runs),
        "promptResolvedCount": len(runs) - missing_prompt_count,
        "missingPromptCount": missing_prompt_count,
        "variantCounts": dict(sorted(variant_counts.items())),
        "languageCounts": dict(sorted(language_counts.items())),
        "projectCounts": dict(sorted(project_counts.items())),
    }


def _increment(counts: dict[str, int], key: str) -> None:
    counts[key] = counts.get(key, 0) + 1


def _safe_id(value: str) -> str:
    return "".join(
        character if character.isalnum() or character in {".", "-", "_"} else "-"
        for character in value
    ).strip("-") or "session"
