"""Large-library optimization analysis tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.large_library_optimization_analysis import (
    build_large_library_optimization_analysis,
)
from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.large_library_report_chain import (
    build_large_library_report_chain,
)

_ROOT = Path(__file__).resolve().parents[3]


def test_large_library_optimization_analysis_collects_missing_results() -> None:
    report_chain = build_large_library_report_chain(_ROOT)

    analysis = build_large_library_optimization_analysis(report_chain)

    _validate_schema(analysis)
    assert analysis["summary"] == {
        "status": "collecting",
        "expectedVariantRunCount": 60,
        "observedVariantRunCount": 0,
        "missingVariantRunCount": 60,
        "findingCount": 1,
    }
    assert analysis["findings"][0]["kind"] == "missing-variant-results"
    assert analysis["improvementPlan"][0]["id"] == "collect-variant-receipts"
    assert len(analysis["collectionManifest"]["expectedVariantRuns"]) == 60
    assert len(analysis["collectionManifest"]["collectionRuns"]) == 60
    assert len(analysis["collectionManifest"]["runsNeedingVariantReceipt"]) == 60
    assert len(analysis["collectionManifest"]["missingVariantRuns"]) == 60
    assert analysis["collectionManifest"]["collectionStatus"] == "collecting"
    assert analysis["collectionManifest"]["metricSourceCounts"] == {"missing": 60}
    assert analysis["collectionManifest"]["missingVariantRuns"][0][
        "variantRunId"
    ].endswith(":no-package-cohesion")
    assert (
        analysis["collectionManifest"]["missingVariantRuns"][0]["collectionStatus"]
        == "missing-result"
    )


def test_large_library_optimization_analysis_aggregates_variant_results() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    results = _variant_results(report_chain)

    analysis = build_large_library_optimization_analysis(report_chain, results)

    _validate_schema(analysis)
    assert analysis["summary"] == {
        "status": "analyzed",
        "expectedVariantRunCount": 60,
        "observedVariantRunCount": 60,
        "missingVariantRunCount": 0,
        "findingCount": 0,
    }
    assert analysis["improvementPlan"][0]["id"] == "calibrate-query-first-stage"
    assert analysis["improvementPlan"][0]["overallWinner"] == "no-package-cohesion"
    assert analysis["variantRecommendations"]["status"] == "ready"
    assert analysis["variantRecommendations"]["overallWinner"][
        "ablationVariant"
    ] == "no-package-cohesion"
    assert [
        item["ablationVariant"]
        for item in analysis["variantRecommendations"]["overallRank"]
    ] == [
        "no-package-cohesion",
        "no-query-clause-coverage",
        "no-query-seed-prior",
    ]
    assert analysis["variantRecommendations"]["bucketWinners"]
    assert analysis["variantRecommendations"]["adaptivePolicy"][0][
        "ablationVariant"
    ] == "no-package-cohesion"
    first_bucket = analysis["variantRecommendations"]["bucketWinners"][0]
    assert first_bucket["evidence"]["granularity"] == "scenario-receipt"
    assert first_bucket["evidence"]["scenarioIds"]
    assert first_bucket["evidence"]["questionIds"]
    assert analysis["collectionManifest"]["missingVariantRuns"] == []
    assert analysis["collectionManifest"]["runsNeedingVariantReceipt"] == []
    assert analysis["collectionManifest"]["collectionStatus"] == "collected"
    assert analysis["collectionManifest"]["metricSourceCounts"] == {
        "variant-result-packet": 60
    }
    assert len(analysis["collectionManifest"]["observedVariantRunIds"]) == 60
    assert _aggregation_count(analysis, "rust", "deep") >= 3
    assert _aggregation_count(analysis, "typescript", "strict") == 6


def test_large_library_optimization_analysis_collects_derived_receipts() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    results = _variant_results(report_chain)
    results[0]["receiptMetrics"]["metricSource"] = "source-sandtable-receipt"
    results[1]["receiptMetrics"]["metricSource"] = "fallback"
    results[2]["receiptMetrics"][
        "metricSource"
    ] = "source-equivalent-variant-receipt"

    analysis = build_large_library_optimization_analysis(report_chain, results)

    _validate_schema(analysis)
    assert analysis["summary"] == {
        "status": "collecting",
        "expectedVariantRunCount": 60,
        "observedVariantRunCount": 60,
        "missingVariantRunCount": 0,
        "findingCount": 3,
    }
    assert [item["kind"] for item in analysis["findings"]] == [
        "baseline-derived-variant-results",
        "source-equivalent-variant-results",
        "fallback-derived-variant-results",
    ]
    assert analysis["improvementPlan"][0] == {
        "id": "collect-variant-receipts",
        "priority": "p0",
        "action": "run ablation-specific sandtable receipts before graph calibration",
        "targetMetric": "runsNeedingVariantReceipt",
    }
    assert analysis["collectionManifest"]["missingVariantRuns"] == []
    assert len(analysis["collectionManifest"]["runsNeedingVariantReceipt"]) == 3
    statuses = {
        item["collectionStatus"]
        for item in analysis["collectionManifest"]["runsNeedingVariantReceipt"]
        if isinstance(item, dict)
    }
    assert statuses == {
        "baseline-derived",
        "source-equivalent",
        "fallback-derived",
    }
    assert analysis["collectionManifest"]["metricSourceCounts"] == {
        "fallback": 1,
        "source-sandtable-receipt": 1,
        "source-equivalent-variant-receipt": 1,
        "variant-result-packet": 57,
    }


def test_large_library_optimization_analysis_cli_emits_json(
    tmp_path: Path,
    capsys,
) -> None:
    report_chain_path = _write_report_chain(tmp_path)

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-optimization-analysis",
                str(report_chain_path),
                "--json",
            ]
        )
        == 0
    )

    payload = json.loads(capsys.readouterr().out)
    _validate_schema(payload)
    assert payload["summary"]["status"] == "collecting"


def test_large_library_optimization_analysis_cli_writes_output(
    tmp_path: Path,
    capsys,
) -> None:
    report_chain_path = _write_report_chain(tmp_path)
    output_path = tmp_path / "analysis.json"

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-optimization-analysis",
                str(report_chain_path),
                "--output",
                str(output_path),
            ]
        )
        == 0
    )

    assert capsys.readouterr().out == ""
    payload = json.loads(output_path.read_text(encoding="utf-8"))
    _validate_schema(payload)
    assert payload["summary"]["status"] == "collecting"


def test_large_library_optimization_analysis_cli_fails_on_missing(
    tmp_path: Path,
    capsys,
) -> None:
    report_chain_path = _write_report_chain(tmp_path)

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-optimization-analysis",
                str(report_chain_path),
                "--fail-on-missing",
            ]
        )
        == 1
    )

    output = capsys.readouterr().out
    assert output.startswith("[large-library-optimization-analysis] ")
    assert "missing=60" in output
    assert "|missing variantRunId=" in output


def _variant_results(report_chain: dict[str, object]) -> list[dict[str, object]]:
    results = []
    for run in report_chain["optimizationMatrix"]:
        assert isinstance(run, dict)
        for variant in run["ablationVariants"]:
            results.append(
                {
                    "runId": run["runId"],
                    "language": run["language"],
                    "package": run["package"],
                    "depthBucket": run["depthBucket"],
                    "ablationVariant": variant,
                    "receiptMetrics": {
                        "aspCommandCount": run["maxAspCommands"],
                        "searchCommandCount": max(1, run["maxAspCommands"] - 2),
                        "queryCommandCount": 2,
                        "repeatedCommandCount": 0,
                    "commandsToFirstUsefulLocator": 2,
                    "frontierFollowRate": 1.0,
                    "rawReadFallbackCount": 0,
                    "duplicateSelectorCount": 0,
                    "sameOwnerScanCount": 0,
                    "elapsedMs": _elapsed_ms_for_variant(variant),
                    "stdoutBytes": 1000,
                    "stderrBytes": 0,
                },
                    "answerMetrics": {
                        "finalAnswerStatus": "answered",
                        "answerQualityJudgment": 0.9,
                        "missingEvidenceCount": 0,
                        "wrongOwnerCount": 0,
                    },
                }
            )
    return results


def _elapsed_ms_for_variant(variant: object) -> int:
    return {
        "no-package-cohesion": 10,
        "no-query-clause-coverage": 20,
        "no-query-seed-prior": 30,
    }.get(str(variant), 99)


def _write_report_chain(tmp_path: Path) -> Path:
    report_chain = build_large_library_report_chain(_ROOT)
    path = tmp_path / "large-library-report-chain.json"
    path.write_text(json.dumps(report_chain), encoding="utf-8")
    return path


def _aggregation_count(
    analysis: dict[str, object], language: str, depth_bucket: str
) -> int:
    return sum(
        1
        for item in analysis["aggregations"]
        if isinstance(item, dict)
        and item["language"] == language
        and item["depthBucket"] == depth_bucket
    )


def _validate_schema(packet: dict[str, object]) -> None:
    schema = json.loads(
        (
            _ROOT
            / "schemas"
            / "semantic-sandtable-large-library-optimization-analysis.v1.schema.json"
        ).read_text(encoding="utf-8")
    )
    Draft202012Validator(schema).validate(packet)
