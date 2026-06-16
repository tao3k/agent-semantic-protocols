"""Adaptive graph-turbo simulation runner tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.large_library_adaptive_simulation import (
    run_large_library_adaptive_simulation,
)


def test_adaptive_simulation_writes_question_plan_and_validation(
    tmp_path: Path,
) -> None:
    repo_root = _repo_root(tmp_path)
    output_root = tmp_path / "out"

    report = run_large_library_adaptive_simulation(
        repo_root,
        _policy(),
        _manifest(repo_root),
        output_root,
        runner=_fake_runner,
    )

    _validate_schema(repo_root, report)
    assert report["summary"]["runCount"] == 1
    assert report["summary"]["statusCounts"] == {"pass": 1}
    assert report["summary"]["queryQualityCounts"] == {"low": 1}
    assert report["summary"]["packageCohesionCounts"] == {"low": 1}
    assert report["summary"]["thirdStepActionCounts"] == {"owner-items": 1}
    assert report["summary"]["ownerItemsRecoveryCounts"] == {"scoped-rg-query": 1}
    assert report["summary"]["ownerItemsTransitionCounts"] == {"owner-items-miss": 1}
    assert report["summary"]["selectorQualityCounts"] == {"not-selector-ready": 1}
    assert report["summary"]["finalStepActionCounts"] == {"rg-query": 1}
    assert report["summary"]["finalRecommendedNextCounts"] == {
        "A1.scoped-rg-query": 1
    }
    assert report["summary"]["finalOwnerItemsRecoveryCounts"] == {
        "not-owner-items": 1
    }
    assert report["summary"]["finalOwnerItemsTransitionCounts"] == {"search-refine": 1}
    assert report["summary"]["finalSelectorQualityCounts"] == {
        "not-selector-ready": 1
    }
    assert report["summary"]["totalCommandCount"] == 4
    assert report["runReports"][0]["thirdStepSignals"] == {
        "action": "owner-items",
        "ownerItemsRecovery": "scoped-rg-query",
        "ownerItemsTransition": "owner-items-miss",
        "selectorQuality": "not-selector-ready",
        "recommendedNext": "scoped-rg-query",
        "nextCommandClass": "rg-query",
    }
    assert report["runReports"][0]["finalStepSignals"] == {
        "action": "rg-query",
        "ownerItemsRecovery": "not-owner-items",
        "ownerItemsTransition": "search-refine",
        "selectorQuality": "not-selector-ready",
        "recommendedNext": "A1.scoped-rg-query",
        "nextCommandClass": "rg-query",
    }
    assert [item["id"] for item in report["algorithmImprovementPlan"]] == [
        "graph-turbo.seed-query-quality",
        "graph-turbo.package-cohesion",
        "graph-turbo.selector-precision",
        "graph-turbo.owner-items-recovery",
        "graph-turbo.final-frontier-convergence",
    ]
    assert report["ownerItemsRecoveryCases"] == [
        {
            "runId": "run-a",
            "scenarioId": "rust.demo",
            "questionId": "demo-owner",
            "language": "rust",
            "ablationVariant": "no-package-cohesion",
            "projectName": "demo",
            "recovery": "scoped-rg-query",
            "owner": "src/lib.rs",
            "query": "demo|owner",
            "reason": "no-owner-item-match",
            "nextCommand": "asp rg -query 'demo|owner' src/lib.rs",
        }
    ]
    assert report["selectorQualityCases"] == []
    validation = json.loads(Path(report["validationReportPath"]).read_text())
    assert validation["status"] == "complete"
    aggregate = json.loads(Path(report["questionPlanAggregatePath"]).read_text())
    assert aggregate["rollup"]["questionCount"] == 1


def test_adaptive_simulation_reports_selector_quality_cases(
    tmp_path: Path,
) -> None:
    repo_root = _repo_root(tmp_path)
    output_root = tmp_path / "out"

    report = run_large_library_adaptive_simulation(
        repo_root,
        _policy(),
        _manifest(repo_root),
        output_root,
        runner=_fake_selector_runner,
    )

    _validate_schema(repo_root, report)
    assert report["summary"]["ownerItemsRecoveryCounts"] == {"selector-ready": 1}
    assert report["summary"]["selectorQualityCounts"] == {
        "secondary-artifact-selector": 1
    }
    assert "graph-turbo.selector-quality" in [
        item["id"] for item in report["algorithmImprovementPlan"]
    ]
    assert report["selectorQualityCases"] == [
        {
            "runId": "run-a",
            "scenarioId": "rust.demo",
            "questionId": "demo-owner",
            "language": "rust",
            "ablationVariant": "no-package-cohesion",
            "projectName": "demo",
            "selectorQuality": "secondary-artifact-selector",
            "owner": "tests/cases/demo.ts",
            "query": "module|resolution",
            "selector": "tests/cases/demo.ts:10:10",
            "matchedQueryTerms": ["module"],
            "missingQueryTerms": ["resolution"],
            "reason": "owner-item-selector-ready",
            "nextCommand": "asp typescript query --selector tests/cases/demo.ts:10:10 --workspace . --code",
        }
    ]
    assert report["runReports"][0]["finalStepSignals"] == {
        "action": "owner-items",
        "ownerItemsRecovery": "selector-ready",
        "ownerItemsTransition": "selector-ready",
        "selectorQuality": "secondary-artifact-selector",
        "recommendedNext": "query-selector",
        "nextCommandClass": "query-code",
    }


def test_adaptive_simulation_follows_rg_wrapper_to_fd_frontier(
    tmp_path: Path,
) -> None:
    repo_root = _repo_root(tmp_path)
    output_root = tmp_path / "out"

    report = run_large_library_adaptive_simulation(
        repo_root,
        _policy(),
        _manifest(repo_root),
        output_root,
        runner=_fake_rg_followup_runner,
    )

    _validate_schema(repo_root, report)
    assert report["summary"]["totalCommandCount"] == 5
    assert report["runReports"][0]["thirdStepSignals"] == {
        "action": "rg-query",
        "ownerItemsRecovery": "not-owner-items",
        "ownerItemsTransition": "search-refine",
        "selectorQuality": "not-selector-ready",
        "recommendedNext": "A1.fd-query",
        "nextCommandClass": "fd-query",
    }
    assert report["runReports"][0]["finalStepSignals"] == {
        "action": "fd-query",
        "ownerItemsRecovery": "not-owner-items",
        "ownerItemsTransition": "owner-items-ready",
        "selectorQuality": "not-selector-ready",
        "recommendedNext": "A1.owner-items",
        "nextCommandClass": "owner-items",
    }
    assert report["summary"]["finalOwnerItemsTransitionCounts"] == {"owner-items-ready": 1}


def _repo_root(tmp_path: Path) -> Path:
    schema_root = Path(__file__).resolve().parents[3] / "schemas"
    scenario_path = tmp_path / "sandtables" / "rust" / "demo.json"
    scenario_path.parent.mkdir(parents=True)
    scenario_path.write_text(
        json.dumps(
            {
                "id": "rust.demo",
                "language": "rust",
                "workdir": ".",
                "evidence": {"targetLibrary": {"package": "demo"}},
            }
        ),
        encoding="utf-8",
    )
    (tmp_path / "schemas").symlink_to(schema_root)
    return tmp_path


def _policy() -> dict[str, object]:
    run = _planned_run()
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-adaptive-query-policy",
        "packetKind": "graph-turbo-adaptive-query-policy",
        "status": "ready",
        "defaultPolicy": {"ablationVariant": run["ablationVariant"]},
        "validationPlan": {"runCount": 1, "runs": [run]},
    }


def _manifest(repo_root: Path) -> dict[str, object]:
    run = {
        **_planned_run(),
        "prompt": "Find the demo owner before editing.",
        "promptResolved": True,
        "language": "rust",
        "project": {"name": "demo", "source": "registry"},
        "sessionRoot": str(repo_root / "session"),
        "commandArgs": [],
        "expectedArtifacts": {},
    }
    return {"runs": [run]}


def _planned_run() -> dict[str, object]:
    return {
        "runId": "run-a",
        "scenarioId": "rust.demo",
        "scenarioPath": "sandtables/rust/demo.json",
        "questionId": "demo-owner",
        "ablationVariant": "no-package-cohesion",
        "env": {"ASP_GRAPH_TURBO_ABLATION_VARIANT": "no-package-cohesion"},
        "expectedReceiptGranularity": "per-question-live-agent",
    }


def _fake_runner(
    command: list[str],
    _workdir: Path,
    _env: dict[str, str],
) -> dict[str, object]:
    if "pipe" in command:
        return {
            "exitCode": 0,
            "stdout": "\n".join(
                [
                    "[search-pipe] lang=rust",
                    "queryQuality=low reason=package-drift",
                    "packageCohesion=low packages=demo,other",
                    "risk=package-drift",
                    "recommendedNext=A1.rg-query-set",
                    "nextCommand=asp rust search owner src/lib.rs items --query 'demo|owner' --view seeds .",
                ]
            ),
            "stderr": "",
        }
    if "owner" in command and "items" in command:
        return {
            "exitCode": 0,
            "stdout": "\n".join(
                [
                    "[search-reasoning] q=owner-query",
                    "recommendedNext=scoped-rg-query",
                    "nextCommand=asp rg -query 'demo|owner' src/lib.rs",
                    "reason=no-owner-item-match",
                ]
            ),
            "stderr": "",
        }
    if len(command) > 1 and command[1] == "rg":
        return {
            "exitCode": 0,
            "stdout": "\n".join(
                [
                    "[search-rg] view=seeds",
                    "recommendedNext=A1.scoped-rg-query",
                    "nextCommand=asp rg -query 'demo|owner' src/lib.rs",
                ]
            ),
            "stderr": "",
        }
    return {"exitCode": 0, "stdout": "[search-prime] ok", "stderr": ""}


def _fake_selector_runner(
    command: list[str],
    _workdir: Path,
    _env: dict[str, str],
) -> dict[str, object]:
    if "pipe" in command:
        return {
            "exitCode": 0,
            "stdout": "\n".join(
                [
                    "[search-pipe] lang=rust",
                    "queryQuality=low reason=package-drift",
                    "packageCohesion=low packages=demo,tests",
                    "risk=package-drift",
                    "recommendedNext=A1.owner-items",
                    "nextCommand=asp rust search owner tests/cases/demo.ts items --query 'module|resolution' --view seeds .",
                ]
            ),
            "stderr": "",
        }
    if "owner" in command and "items" in command:
        return {
            "exitCode": 0,
            "stdout": "\n".join(
                [
                    "[search-reasoning] q=owner-query",
                    "syntax I selector=tests/cases/demo.ts:10:10 pattern='module'",
                    "recommendedNext=query-selector",
                    "nextCommand=asp typescript query --selector tests/cases/demo.ts:10:10 --workspace . --code",
                    "reason=owner-item-selector-ready",
                ]
            ),
            "stderr": "",
        }
    return {"exitCode": 0, "stdout": "[search-prime] ok", "stderr": ""}


def _fake_rg_followup_runner(
    command: list[str],
    _workdir: Path,
    _env: dict[str, str],
) -> dict[str, object]:
    if "pipe" in command:
        return {
            "exitCode": 0,
            "stdout": "\n".join(
                [
                    "[search-pipe] lang=rust",
                    "queryQuality=low reason=package-drift",
                    "packageCohesion=low packages=demo,other",
                    "risk=package-drift",
                    "recommendedNext=A1.rg-query-set",
                    "nextCommand=asp rg -query 'demo owner' -query 'fd frontier' .",
                ]
            ),
            "stderr": "",
        }
    if len(command) > 1 and command[1] == "rg":
        return {
            "exitCode": 0,
            "stdout": "\n".join(
                [
                    "[search-rg] view=seeds",
                    "recommendedNext=A1.fd-query",
                    "nextCommand=asp fd -query 'demo|owner|frontier' .",
                ]
            ),
            "stderr": "",
        }
    if len(command) > 1 and command[1] == "fd":
        return {
            "exitCode": 0,
            "stdout": "\n".join(
                [
                    "[search-fd] view=seeds",
                    "recommendedNext=A1.owner-items",
                    "nextCommand=asp rust search owner src/lib.rs items --query 'demo|owner' --view seeds .",
                ]
            ),
            "stderr": "",
        }
    return {"exitCode": 0, "stdout": "[search-prime] ok", "stderr": ""}


def _validate_schema(repo_root: Path, packet: dict[str, object]) -> None:
    schema = json.loads(
        (
            repo_root
            / "schemas"
            / "semantic-graph-turbo-adaptive-simulation-report.v1.schema.json"
        ).read_text()
    )
    Draft202012Validator(schema).validate(packet)
