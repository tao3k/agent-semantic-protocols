# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Large-library report chain readiness tests."""

from __future__ import annotations

import json
from pathlib import Path

from unit.schema_validation import schema_validator_for

from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.large_library_report_chain import (
    build_large_library_report_chain,
)

_ROOT = Path(__file__).resolve().parents[3]


def test_large_library_report_chain_can_pass_with_multi_depth_ts_rust_fixture(
    tmp_path: Path,
) -> None:
    scenarios = []
    for language in ("rust", "typescript"):
        path = tmp_path / f"{language}.json"
        path.write_text(json.dumps(_scenario(language)), encoding="utf-8")
        scenarios.append(path)

    report = build_large_library_report_chain(
        tmp_path,
        scenarios,
        languages=("rust", "typescript"),
    )

    _validate_schema(report)
    assert report["optimizationGate"]["status"] == "pass"
    assert report["rollup"]["readyLanguageCount"] == 2
    assert report["rollup"]["optimizationRunCount"] == 6
    assert report["rollup"]["optimizationVariantRunCount"] == 30
    assert report["optimizationBatch"]["readyToCollectReceipts"] is True
    assert report["benchmarkData"] == {
        "scenarioCount": 2,
        "searchCommandCount": 2,
        "uniqueSearchCommandCount": 2,
        "optimizationRunCount": 6,
        "optimizationVariantRunCount": 30,
        "ablationVariantCount": 5,
        "coveredSearchMethods": ["search/playbook"],
        "coveredSearchQueries": ["feature", "owner"],
        "byLanguage": [
            _expected_language_benchmark("rust"),
            _expected_language_benchmark("typescript"),
        ],
    }
    assert report["searchCommandSet"] == [
        _expected_playbook_command_entry("rust"),
        _expected_playbook_command_entry("typescript"),
    ]
    assert report["findings"] == []
    assert all(entry["findings"] == [] for entry in report["languages"])
    assert _matrix_targets_query_first_stage(report)


def test_large_library_report_chain_blocks_ambient_asp_binary_fixture(
    tmp_path: Path,
) -> None:
    scenarios = []
    for language in ("rust", "typescript"):
        scenario = _scenario(language)
        if language == "rust":
            scenario["observation"] = {
                "commandFlow": {
                    "aspBinaryProvenance": {
                        "commandCount": 1,
                        "workspaceBinaryCommands": 0,
                        "freshnessRiskCommands": 1,
                        "kindCounts": {"ambient-path": 1},
                        "tokens": {"asp": 1},
                    }
                }
            }
        path = tmp_path / f"{language}.json"
        path.write_text(json.dumps(scenario), encoding="utf-8")
        scenarios.append(path)

    report = build_large_library_report_chain(
        tmp_path,
        scenarios,
        languages=("rust", "typescript"),
    )

    _validate_schema(report)
    assert report["optimizationGate"]["status"] == "review"
    assert report["optimizationGate"]["blockingFindingCount"] == 1
    assert report["rollup"]["aspBinaryFreshnessRiskCommandCount"] == 1
    assert report["rollup"]["aspBinaryFreshnessRiskScenarioCount"] == 1
    by_language = {entry["language"]: entry for entry in report["languages"]}
    assert _finding_kinds(by_language["rust"]) == {"asp-binary-freshness-risk"}
    assert _finding_kinds(by_language["typescript"]) == set()


def test_large_library_report_chain_cli_emits_json(capsys) -> None:
    assert (
        main(["--repo-root", str(_ROOT), "--large-library-report-chain", "--json"]) == 0
    )

    payload = json.loads(capsys.readouterr().out)
    _validate_schema(payload)
    assert payload["optimizationGate"]["status"] == "pass"


def test_large_library_report_chain_cli_passes_fail_on_missing_when_ready(
    capsys,
) -> None:
    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-report-chain",
                "--fail-on-missing",
            ]
        )
        == 0
    )

    output = capsys.readouterr().out
    assert output.startswith("[large-library-report-chain] ")
    assert "gate=pass" in output
    assert "runs=26" in output
    assert "variantRuns=130" in output


def _scenario(language: str) -> dict[str, object]:
    return {
        "id": f"{language}.multi-depth",
        "language": language,
        "coverage": ["large-library"],
        "workdir": ".",
        "evidence": {
            "source": "unit-test",
            "fixtureTier": "large-library",
            "targetLibrary": {
                "language": language,
                "name": f"{language}-lib",
                "package": f"{language}-lib",
                "repository": f"example/{language}-lib",
                "workdirKind": "checkout",
            },
            "intentCases": [
                {
                    "intentKind": "feature-implementation",
                    "intent": "feature",
                    "stepIds": ["query"],
                    "queryTerms": ["feature"],
                }
            ],
            "deepQuestionCases": [
                _question("strict", 3),
                _question("medium", 6),
                _question("deep", 8),
            ],
        },
        "steps": [
            {
                "id": "query",
                "command": _scenario_command(language),
            }
        ],
    }


def _scenario_command(language: str) -> list[str]:
    return [
        "asp",
        "search",
        "playbook",
        "--language",
        language,
        "--rg",
        "-n",
        "-e",
        "feature|owner",
        ".",
        "--tantivy",
        'title:"feature"^2 OR body:"owner"',
    ]


def _question(question_id: str, max_asp_commands: int) -> dict[str, object]:
    return {
        "id": question_id,
        "question": f"{question_id} question",
        "stepIds": ["query"],
        "queryTerms": ["feature", question_id, "owner"],
        "audit": {
            "maxAspCommands": max_asp_commands,
            "maxSearchCommands": min(max_asp_commands, 4),
            "maxQueryCommands": min(max_asp_commands, 3),
            "maxRepeatedCommands": 0,
            "requiresGraphSignals": True,
            "requiresQuerySet": True,
            "requiresHookEvents": True,
            "requiresComplexCommandFlow": question_id == "strict",
            "requiresTokenCost": question_id == "strict",
        },
        "expectedAspFlow": {
            "requiredStages": ["search-prime", "search-pipe", "query-selector"],
            "forbiddenStages": ["repeated-prime", "repeated-commands"],
        },
    }


def _validate_schema(report: dict[str, object]) -> None:
    schema_validator_for(
        _ROOT
        / "schemas"
        / "semantic-sandtable-large-library-report-chain.v1.schema.json"
    ).validate(report)


def _finding_kinds(language_entry: dict[str, object]) -> set[str]:
    return {
        str(finding["kind"])
        for finding in language_entry["findings"]
        if isinstance(finding, dict)
    }


def _language_benchmark_counts(report: dict[str, object]) -> dict[str, tuple[int, ...]]:
    return {
        str(entry["language"]): (
            int(entry["scenarioCount"]),
            int(entry["searchCommandCount"]),
            int(entry["uniqueSearchCommandCount"]),
            int(entry["optimizationRunCount"]),
            int(entry["optimizationVariantRunCount"]),
        )
        for entry in report["benchmarkData"]["byLanguage"]
        if isinstance(entry, dict)
    }


def _assert_no_legacy_search_commands(report: dict[str, object]) -> None:
    for entry in report["searchCommandSet"]:
        assert isinstance(entry, dict)
        assert entry["method"] == "search/playbook"
        command = entry["command"]
        assert isinstance(command, list)
        assert command[:3] == ["asp", "search", "playbook"]
        assert command[command.index("--language") + 1] == entry["language"]
        assert "--rg" in command
        assert "--tantivy" in command
        assert "--query-set" not in command
        assert "--view" not in command


def _expected_language_benchmark(language: str) -> dict[str, object]:
    return {
        "language": language,
        "scenarioCount": 1,
        "searchCommandCount": 1,
        "uniqueSearchCommandCount": 1,
        "optimizationRunCount": 3,
        "optimizationVariantRunCount": 15,
        "coveredSearchMethods": ["search/playbook"],
        "coveredSearchQueries": ["feature", "owner"],
    }


def _expected_playbook_command_entry(language: str) -> dict[str, object]:
    command = _scenario_command(language)
    return {
        "commandId": " ".join(command),
        "language": language,
        "method": "search/playbook",
        "view": "playbook",
        "queries": ["feature", "owner"],
        "command": command,
        "scenarioIds": [f"{language}.multi-depth"],
    }


def _matrix_depth_counts(report: dict[str, object]) -> dict[str, dict[str, int]]:
    counts: dict[str, dict[str, int]] = {}
    for entry in report["optimizationMatrix"]:
        if not isinstance(entry, dict):
            continue
        language = str(entry["language"])
        depth = str(entry["depthBucket"])
        counts.setdefault(language, {})
        counts[language][depth] = counts[language].get(depth, 0) + 1
    return counts


def _matrix_targets_query_first_stage(report: dict[str, object]) -> bool:
    expected_variants = {
        "no-query-seed-prior",
        "no-package-cohesion",
        "no-query-clause-coverage",
        "no-local-evidence",
        "no-topology-membership",
    }
    return all(
        isinstance(entry, dict)
        and entry["targetGraphPhase"] == "query-first-stage"
        and set(entry["ablationVariants"]) == expected_variants
        for entry in report["optimizationMatrix"]
    )
