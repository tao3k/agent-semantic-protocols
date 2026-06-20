"""Report-chain gates for graph turbo sandtable summaries."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from unit.asp_graph_turbo_cli_support import (
    sample_graph_turbo_request,
    validate_shared_schema,
)


def test_graph_turbo_sandtable_summary_consumes_large_library_report_chain(
    tmp_path: Path,
) -> None:
    packet_path, benchmark_path = _benchmark_fixture_paths(tmp_path)
    report_chain_path = _report_chain_path(tmp_path, status="pass")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "sandtable-summary",
            "--benchmark",
            str(benchmark_path),
            "--receipt",
            str(_receipt_fixtures_path()),
            "--large-library-report-chain",
            str(report_chain_path),
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(completed.stdout)
    validate_shared_schema(
        payload,
        "semantic-graph-turbo-sandtable-summary.v1.schema.json",
    )

    assert packet_path.exists()
    assert payload["largeLibraryReportChain"] == {
        "schemaId": (
            "agent.semantic-protocols."
            "semantic-sandtable-large-library-report-chain"
        ),
        "packetKind": "large-library-report-chain",
        "languageCount": 2,
        "libraryCount": 8,
        "scenarioCount": 11,
        "deepQuestionCount": 15,
        "readyLanguageCount": 2,
        "optimizationRunCount": 15,
        "optimizationVariantRunCount": 60,
        "optimizationAblationVariantCount": 4,
        "optimizationAblationVariants": [
            "no-query-seed-prior",
            "no-package-cohesion",
            "no-query-clause-coverage",
            "no-local-evidence",
        ],
        "localEvidenceAblationEnabled": True,
        "findingCount": 0,
        "status": "pass",
        "reason": "report chain has multi-depth TS/Rust evidence",
        "blockingFindingCount": 0,
    }
    assert payload["qualityGate"]["status"] == "pass"


def test_graph_turbo_sandtable_summary_gate_blocks_unready_report_chain(
    tmp_path: Path,
) -> None:
    _, benchmark_path = _benchmark_fixture_paths(tmp_path)
    report_chain_path = _report_chain_path(tmp_path, status="review")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "sandtable-summary",
            "--benchmark",
            str(benchmark_path),
            "--receipt",
            str(_receipt_fixtures_path()),
            "--large-library-report-chain",
            str(report_chain_path),
            "--fail-on-gate",
            "--format",
            "json",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    payload = json.loads(completed.stdout)

    assert completed.returncode == 1
    assert payload["qualityGate"]["status"] == "fail"
    assert {
        "field": "largeLibraryReportChain.status",
        "actual": "review",
        "expected": "value == pass",
    } in payload["qualityGate"]["failures"]


def test_graph_turbo_sandtable_summary_text_includes_report_chain(
    tmp_path: Path,
) -> None:
    _, benchmark_path = _benchmark_fixture_paths(tmp_path)
    report_chain_path = _report_chain_path(tmp_path, status="pass")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "sandtable-summary",
            "--benchmark",
            str(benchmark_path),
            "--receipt",
            str(_receipt_fixtures_path()),
            "--large-library-report-chain",
            str(report_chain_path),
            "--format",
            "text",
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    assert "\nlargeLibraryReportChain=status=pass" in completed.stdout
    assert "ready=2" in completed.stdout
    assert "questions=15" in completed.stdout
    assert "runs=15" in completed.stdout
    assert "variantRuns=60" in completed.stdout
    assert "ablationVariants=4" in completed.stdout
    assert "localEvidenceAblation=True" in completed.stdout


def _benchmark_fixture_paths(tmp_path: Path) -> tuple[Path, Path]:
    packet_path = tmp_path / "graph-turbo-request.json"
    benchmark_path = tmp_path / "graph-turbo-benchmark.json"
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")
    benchmark_path.write_text(_benchmark_stdout(packet_path), encoding="utf-8")
    return packet_path, benchmark_path


def _benchmark_stdout(packet_path: Path) -> str:
    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "benchmark",
            str(packet_path),
            "--runs",
            "2",
            "--warmup-runs",
            "0",
            "--cache-mode",
            "disabled",
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    return completed.stdout


def _receipt_fixtures_path() -> Path:
    return (
        Path(__file__).resolve().parents[2]
        / "schemas"
        / ("semantic-fact-frontier-receipt.fixtures.v1.json")
    )


def _report_chain_path(tmp_path: Path, *, status: str) -> Path:
    blocking_count = 0 if status == "pass" else 2
    ready_count = 2 if status == "pass" else 0
    finding_count = 0 if status == "pass" else 5
    path = tmp_path / f"large-library-report-chain-{status}.json"
    path.write_text(
        json.dumps(
            {
                "schemaId": (
                    "agent.semantic-protocols."
                    "semantic-sandtable-large-library-report-chain"
                ),
                "schemaVersion": "1",
                "packetKind": "large-library-report-chain",
                "languages": [],
                "rollup": {
                    "languageCount": 2,
                    "scenarioCount": 11,
                    "libraryCount": 8,
                    "deepQuestionCount": 15,
                    "readyLanguageCount": ready_count,
                    "optimizationRunCount": 15,
                    "optimizationVariantRunCount": 60,
                    "findingCount": finding_count,
                },
                "optimizationBatch": {
                    "targetGraphPhase": "query-first-stage",
                    "nextStage": "collect-receipts",
                    "readyToCollectReceipts": status == "pass",
                    "runCount": 15,
                    "ablationVariantCount": 4,
                    "variantRunCount": 60,
                    "ablationVariants": [
                        "no-query-seed-prior",
                        "no-package-cohesion",
                        "no-query-clause-coverage",
                        "no-local-evidence",
                    ],
                    "aggregationAxes": [
                        "language",
                        "package",
                        "depthBucket",
                        "ablationVariant",
                    ],
                    "requiredReceiptMetrics": ["frontierFollowRate"],
                    "requiredAnswerMetrics": ["answerQualityJudgment"],
                },
                "optimizationMatrix": [],
                "findings": [],
                "optimizationGate": {
                    "status": status,
                    "reason": _report_chain_reason(status),
                    "blockingFindingCount": blocking_count,
                },
            }
        ),
        encoding="utf-8",
    )
    return path


def _report_chain_reason(status: str) -> str:
    if status == "pass":
        return "report chain has multi-depth TS/Rust evidence"
    return "collect multi-depth TS/Rust report-chain evidence before tuning"
