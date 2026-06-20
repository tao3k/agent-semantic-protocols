"""Large-library report chain readiness for graph-turbo optimization."""

from __future__ import annotations

from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from .large_library_optimization_matrix import (
    optimization_batch,
    optimization_matrix,
)
from .scenario_io import discover_scenarios, load_scenario
from .utils import dict_value, list_value, require_str, string_list

DEFAULT_LANGUAGES = ("rust", "typescript")
REQUIRED_DEPTH_BUCKETS = ("strict", "medium", "deep")


def build_large_library_report_chain(
    repo_root: Path,
    scenario_paths: list[Path] | None = None,
    *,
    languages: tuple[str, ...] = DEFAULT_LANGUAGES,
) -> dict[str, Any]:
    selected_languages = tuple(sorted(languages))
    paths = scenario_paths or discover_scenarios(repo_root, [])
    scenarios = [
        _scenario_entry(repo_root, path)
        for path in paths
        if _scenario_language(repo_root, path) in selected_languages
    ]
    scenarios = [scenario for scenario in scenarios if scenario is not None]
    language_entries = [
        _language_entry(language, scenarios) for language in selected_languages
    ]
    findings = _findings(language_entries)
    language_entries = _with_language_findings(language_entries, findings)
    matrix = optimization_matrix(scenarios)
    batch = optimization_batch(matrix)
    return {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-large-library-report-chain",
        "schemaVersion": "1",
        "packetKind": "large-library-report-chain",
        "languages": language_entries,
        "rollup": _rollup(language_entries, findings, matrix, batch),
        "optimizationBatch": batch,
        "optimizationMatrix": matrix,
        "findings": findings,
        "optimizationGate": _optimization_gate(findings),
    }


def _scenario_language(repo_root: Path, path: Path) -> str | None:
    try:
        scenario = load_scenario(path, repo_root)
    except Exception:
        return None
    language = scenario.get("language")
    return language if isinstance(language, str) else None


def _scenario_entry(repo_root: Path, path: Path) -> dict[str, Any] | None:
    try:
        scenario = load_scenario(path, repo_root)
    except Exception:
        return None
    evidence = dict_value(scenario.get("evidence"))
    if "large-library" not in string_list(scenario.get("coverage", [])):
        return None
    target = dict_value(evidence.get("targetLibrary"))
    deep_questions = [
        _deep_question_entry(question)
        for question in list_value(evidence.get("deepQuestionCases"))
        if isinstance(question, dict)
    ]
    entry = {
        "scenarioId": require_str(scenario, "id", path.stem),
        "language": require_str(scenario, "language", "unknown"),
        "path": _display_path(repo_root, path),
        "package": _target_package(target),
        "repository": require_str(target, "repository", "unknown"),
        "fixtureTier": evidence.get("fixtureTier") or "live",
        "intentKinds": sorted(_intent_kinds(evidence)),
        "deepQuestions": deep_questions,
        "live": "liveAgent" in scenario,
        "promptOnly": "prompt-only" in string_list(scenario.get("tags", [])),
    }
    binary_provenance = _asp_binary_provenance_from_scenario(scenario)
    if binary_provenance:
        entry["aspBinaryProvenance"] = binary_provenance
    return entry


def _deep_question_entry(question: dict[str, Any]) -> dict[str, Any]:
    audit = dict_value(question.get("audit"))
    expected_flow = dict_value(question.get("expectedAspFlow"))
    max_asp_commands = _int_value(audit.get("maxAspCommands"))
    return {
        "id": require_str(question, "id", "unknown"),
        "depthBucket": _depth_bucket(max_asp_commands),
        "maxAspCommands": max_asp_commands,
        "maxSearchCommands": _int_value(audit.get("maxSearchCommands")),
        "maxQueryCommands": _int_value(audit.get("maxQueryCommands")),
        "requiresQuerySet": audit.get("requiresQuerySet") is True,
        "requiresGraphSignals": audit.get("requiresGraphSignals") is True,
        "requiresHookEvents": audit.get("requiresHookEvents") is True,
        "requiresComplexPipeFlow": audit.get("requiresComplexPipeFlow") is True,
        "requiresTokenCost": audit.get("requiresTokenCost") is True,
        "requiredStages": string_list(expected_flow.get("requiredStages")),
        "forbiddenStages": string_list(expected_flow.get("forbiddenStages")),
    }


def _language_entry(language: str, scenarios: list[dict[str, Any]]) -> dict[str, Any]:
    owned = [scenario for scenario in scenarios if scenario["language"] == language]
    packages = sorted({scenario["package"] for scenario in owned})
    deep_questions = [
        question for scenario in owned for question in scenario["deepQuestions"]
    ]
    depth_counts = Counter(question["depthBucket"] for question in deep_questions)
    signal_counts = Counter()
    for question in deep_questions:
        for signal in (
            "requiresQuerySet",
            "requiresGraphSignals",
            "requiresHookEvents",
            "requiresComplexPipeFlow",
            "requiresTokenCost",
        ):
            if question.get(signal) is True:
                signal_counts[signal] += 1
    binary_provenance = _merge_binary_provenance(
        [
            dict_value(scenario.get("aspBinaryProvenance"))
            for scenario in owned
            if dict_value(scenario.get("aspBinaryProvenance"))
        ]
    )
    entry = {
        "language": language,
        "scenarioCount": len(owned),
        "libraryCount": len(packages),
        "libraries": packages,
        "deepQuestionCount": len(deep_questions),
        "liveDeepQuestionCount": sum(
            len(scenario["deepQuestions"]) for scenario in owned if scenario["live"]
        ),
        "depthBucketCounts": dict(sorted(depth_counts.items())),
        "requiredSignalCounts": dict(sorted(signal_counts.items())),
        "reportChainReady": _report_chain_ready(depth_counts, deep_questions),
    }
    if binary_provenance:
        entry["aspBinaryProvenance"] = binary_provenance
        entry["aspBinaryFreshnessRiskScenarioCount"] = sum(
            1
            for scenario in owned
            if _binary_freshness_risk_commands(
                dict_value(scenario.get("aspBinaryProvenance"))
            )
            > 0
        )
    return entry


def _report_chain_ready(
    depth_counts: Counter[str], deep_questions: list[dict[str, Any]]
) -> bool:
    return (
        all(depth_counts.get(bucket, 0) > 0 for bucket in REQUIRED_DEPTH_BUCKETS)
        and any(question["requiresTokenCost"] for question in deep_questions)
        and any(question["requiresComplexPipeFlow"] for question in deep_questions)
    )


def _findings(language_entries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for entry in language_entries:
        language = str(entry["language"])
        if int(entry["deepQuestionCount"]) < 3:
            findings.append(
                _finding(
                    language,
                    "insufficient-deep-questions",
                    "warning",
                    "add at least three deep questions before tuning graph turbo",
                )
            )
        missing_depths = [
            bucket
            for bucket in REQUIRED_DEPTH_BUCKETS
            if dict_value(entry.get("depthBucketCounts")).get(bucket, 0) == 0
        ]
        if missing_depths:
            findings.append(
                _finding(
                    language,
                    "missing-depth-buckets",
                    "warning",
                    f"missing question depth buckets: {','.join(missing_depths)}",
                )
            )
        if entry.get("reportChainReady") is not True:
            findings.append(
                _finding(
                    language,
                    "report-chain-not-ready",
                    "warning",
                    "collect TS/Rust multi-depth evidence before algorithm tuning",
                )
            )
        binary_risk_commands = _binary_freshness_risk_commands(
            dict_value(entry.get("aspBinaryProvenance"))
        )
        if binary_risk_commands > 0:
            findings.append(
                _finding(
                    language,
                    "asp-binary-freshness-risk",
                    "error",
                    (
                        "sandtable observed ASP commands from ambient or external "
                        f"binaries: {binary_risk_commands}"
                    ),
                )
            )
    return findings


def _with_language_findings(
    language_entries: list[dict[str, Any]], findings: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    by_language: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for finding in findings:
        by_language[str(finding["language"])].append(finding)
    entries: list[dict[str, Any]] = []
    for entry in language_entries:
        enriched = dict(entry)
        enriched["findings"] = list(by_language.get(str(entry["language"]), []))
        entries.append(enriched)
    return entries


def _finding(language: str, kind: str, severity: str, message: str) -> dict[str, Any]:
    return {
        "language": language,
        "kind": kind,
        "severity": severity,
        "message": message,
    }


def _asp_binary_provenance_from_scenario(
    scenario: dict[str, Any],
) -> dict[str, Any]:
    aggregates = [
        dict_value(item.get("aspBinaryProvenance"))
        for item in _walk_mappings(scenario)
        if dict_value(item.get("aspBinaryProvenance"))
    ]
    if aggregates:
        return _merge_binary_provenance(aggregates)
    binaries = [
        dict_value(item.get("aspBinary"))
        for item in _walk_mappings(scenario)
        if dict_value(item.get("aspBinary"))
    ]
    return _binary_provenance_from_binaries(binaries)


def _walk_mappings(value: object) -> list[dict[str, Any]]:
    if isinstance(value, dict):
        items = [value]
        for item in value.values():
            items.extend(_walk_mappings(item))
        return items
    if isinstance(value, list):
        items: list[dict[str, Any]] = []
        for item in value:
            items.extend(_walk_mappings(item))
        return items
    return []


def _merge_binary_provenance(entries: list[dict[str, Any]]) -> dict[str, Any]:
    if not entries:
        return {}
    kind_counts: Counter[str] = Counter()
    token_counts: Counter[str] = Counter()
    command_count = 0
    workspace_count = 0
    risk_count = 0
    for entry in entries:
        command_count += _int_value(entry.get("commandCount"))
        workspace_count += _int_value(entry.get("workspaceBinaryCommands"))
        risk_count += _int_value(entry.get("freshnessRiskCommands"))
        _add_counter_values(kind_counts, dict_value(entry.get("kindCounts")))
        _add_counter_values(token_counts, dict_value(entry.get("tokens")))
    if command_count == 0:
        command_count = sum(kind_counts.values())
    if workspace_count == 0:
        workspace_count = _workspace_binary_commands(kind_counts)
    if risk_count == 0:
        risk_count = _freshness_risk_commands(kind_counts)
    if command_count == 0 and not kind_counts and not token_counts:
        return {}
    return {
        "commandCount": command_count,
        "workspaceBinaryCommands": workspace_count,
        "freshnessRiskCommands": risk_count,
        "kindCounts": dict(sorted(kind_counts.items())),
        "tokens": dict(sorted(token_counts.items())),
    }


def _binary_provenance_from_binaries(
    binaries: list[dict[str, Any]],
) -> dict[str, Any]:
    if not binaries:
        return {}
    kind_counts = Counter(
        str(binary.get("kind")) for binary in binaries if binary.get("kind")
    )
    token_counts = Counter(
        str(binary.get("token")) for binary in binaries if binary.get("token")
    )
    return {
        "commandCount": len(binaries),
        "workspaceBinaryCommands": _workspace_binary_commands(kind_counts),
        "freshnessRiskCommands": _freshness_risk_commands(kind_counts),
        "kindCounts": dict(sorted(kind_counts.items())),
        "tokens": dict(sorted(token_counts.items())),
    }


def _add_counter_values(counter: Counter[str], values: dict[str, Any]) -> None:
    for key, value in values.items():
        count = _int_value(value)
        if count > 0:
            counter[str(key)] += count


def _workspace_binary_commands(kind_counts: Counter[str]) -> int:
    return sum(
        count
        for kind, count in kind_counts.items()
        if kind in {"project-bin", "cargo-target"}
    )


def _freshness_risk_commands(kind_counts: Counter[str]) -> int:
    return sum(
        count
        for kind, count in kind_counts.items()
        if kind not in {"project-bin", "cargo-target"}
    )


def _binary_freshness_risk_commands(binary_provenance: dict[str, Any]) -> int:
    return _int_value(binary_provenance.get("freshnessRiskCommands"))


def _rollup(
    language_entries: list[dict[str, Any]],
    findings: list[dict[str, Any]],
    optimization_matrix: list[dict[str, Any]],
    optimization_batch: dict[str, Any],
) -> dict[str, Any]:
    binary_risk_commands = sum(
        _binary_freshness_risk_commands(dict_value(entry.get("aspBinaryProvenance")))
        for entry in language_entries
    )
    binary_risk_scenarios = sum(
        _int_value(entry.get("aspBinaryFreshnessRiskScenarioCount"))
        for entry in language_entries
    )
    return {
        "languageCount": len(language_entries),
        "scenarioCount": sum(int(entry["scenarioCount"]) for entry in language_entries),
        "libraryCount": sum(int(entry["libraryCount"]) for entry in language_entries),
        "deepQuestionCount": sum(
            int(entry["deepQuestionCount"]) for entry in language_entries
        ),
        "readyLanguageCount": sum(
            1 for entry in language_entries if entry["reportChainReady"] is True
        ),
        "optimizationRunCount": len(optimization_matrix),
        "optimizationVariantRunCount": int(
            optimization_batch["variantRunCount"]
        ),
        "findingCount": len(findings),
        "aspBinaryFreshnessRiskCommandCount": binary_risk_commands,
        "aspBinaryFreshnessRiskScenarioCount": binary_risk_scenarios,
    }


def _optimization_gate(findings: list[dict[str, Any]]) -> dict[str, Any]:
    blocking = [
        finding
        for finding in findings
        if finding["kind"]
        in {
            "report-chain-not-ready",
            "missing-depth-buckets",
            "asp-binary-freshness-risk",
        }
    ]
    return {
        "status": "pass" if not blocking else "review",
        "reason": (
            "report chain has multi-depth TS/Rust evidence"
            if not blocking
            else "collect multi-depth TS/Rust report-chain evidence before tuning"
        ),
        "blockingFindingCount": len(blocking),
    }


def _intent_kinds(evidence: dict[str, Any]) -> set[str]:
    kinds = set()
    for case in list_value(evidence.get("intentCases")):
        intent_kind = dict_value(case).get("intentKind")
        if isinstance(intent_kind, str):
            kinds.add(intent_kind)
    return kinds


def _target_package(target: dict[str, Any]) -> str:
    package = target.get("package")
    if isinstance(package, str) and package:
        return package
    name = target.get("name")
    if isinstance(name, str) and name:
        return name
    return "unknown"


def _depth_bucket(max_asp_commands: int) -> str:
    if max_asp_commands <= 3:
        return "strict"
    if max_asp_commands <= 6:
        return "medium"
    return "deep"


def _int_value(value: object) -> int:
    return value if isinstance(value, int) and not isinstance(value, bool) else 0


def _display_path(repo_root: Path, path: Path) -> str:
    try:
        return str(path.relative_to(repo_root))
    except ValueError:
        return str(path)
