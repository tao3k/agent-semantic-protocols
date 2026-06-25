"""Large-library optimization variant batch tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.large_library_optimization_analysis import (
    build_large_library_optimization_analysis,
)
from tools.semantic_sandtable.large_library_report_chain import (
    build_large_library_report_chain,
)
from tools.semantic_sandtable.large_library_variant_batch import (
    build_large_library_variant_batch,
)

_ROOT = Path(__file__).resolve().parents[3]


def test_large_library_variant_batch_feeds_analysis() -> None:
    report_chain = build_large_library_report_chain(_ROOT)

    packets = build_large_library_variant_batch(
        report_chain,
        receipt_metrics=_receipt_metrics(),
        answer_metrics=_answer_metrics(),
        source_receipt_path="receipts/full-batch.json",
    )
    analysis = build_large_library_optimization_analysis(report_chain, packets)

    assert len(packets) == report_chain["optimizationBatch"]["variantRunCount"]
    assert analysis["summary"]["status"] == "collecting"
    assert analysis["summary"]["observedVariantRunCount"] == len(packets)
    assert analysis["summary"]["missingVariantRunCount"] == 0
    assert analysis["findings"][0]["kind"] == "fallback-derived-variant-results"
    _validate_variant_schema(packets[0])


def test_large_library_variant_batch_cli_writes_output(
    tmp_path: Path,
    capsys,
) -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    report_chain_path = tmp_path / "report-chain.json"
    output_path = tmp_path / "variant-batch.json"
    report_chain_path.write_text(json.dumps(report_chain), encoding="utf-8")

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-variant-batch",
                str(report_chain_path),
                "--output",
                str(output_path),
                *_metric_args("--receipt-metric", _receipt_metrics()),
                *_metric_args("--answer-metric", _answer_metrics()),
            ]
        )
        == 0
    )

    assert capsys.readouterr().out == ""
    packets = json.loads(output_path.read_text(encoding="utf-8"))
    assert len(packets) == report_chain["optimizationBatch"]["variantRunCount"]
    _validate_variant_schema(packets[0])


def test_large_library_variant_batch_derives_metrics_from_sandtable_receipt() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    first_run = report_chain["optimizationMatrix"][0]
    assert isinstance(first_run, dict)
    receipt = _sandtable_receipt(first_run["scenarioId"])

    packets = build_large_library_variant_batch(
        report_chain,
        receipt_metrics=_fallback_receipt_metrics(),
        answer_metrics=_fallback_answer_metrics(),
        sandtable_receipt=receipt,
    )

    first_packet = packets[0]
    assert first_packet["scenarioId"] == first_run["scenarioId"]
    assert first_packet["receiptMetrics"]["aspCommandCount"] == 3
    assert first_packet["receiptMetrics"]["searchCommandCount"] == 2
    assert first_packet["receiptMetrics"]["queryCommandCount"] == 2
    assert first_packet["receiptMetrics"]["repeatedCommandCount"] == 1
    assert first_packet["receiptMetrics"]["commandsToFirstUsefulLocator"] == 2
    assert first_packet["receiptMetrics"]["frontierFollowRate"] == 1.0
    assert first_packet["receiptMetrics"]["elapsedMs"] == 123
    assert first_packet["receiptMetrics"]["stdoutBytes"] == 456
    assert first_packet["receiptMetrics"]["stderrBytes"] == 7
    assert first_packet["receiptMetrics"]["metricSource"] == "source-sandtable-receipt"
    assert first_packet["answerMetrics"]["answerQualityJudgment"] == 1.0


def test_large_library_variant_batch_uses_variant_specific_receipts() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    first_run = report_chain["optimizationMatrix"][0]
    assert isinstance(first_run, dict)

    packets = build_large_library_variant_batch(
        report_chain,
        receipt_metrics=_fallback_receipt_metrics(),
        answer_metrics=_fallback_answer_metrics(),
        sandtable_receipt=_sandtable_receipt(first_run["scenarioId"]),
        variant_sandtable_receipts={
            "no-query-seed-prior": _variant_sandtable_receipt(first_run["scenarioId"])
        },
        variant_source_receipt_paths={"no-query-seed-prior": "variant-prior.json"},
        source_receipt_path="baseline.json",
    )

    variant_packet = packets[0]
    baseline_packet = packets[1]
    assert variant_packet["ablationVariant"] == "no-query-seed-prior"
    assert variant_packet["receiptMetrics"]["aspCommandCount"] == 5
    assert variant_packet["receiptMetrics"]["elapsedMs"] == 777
    assert variant_packet["receiptMetrics"]["metricSource"] == "variant-sandtable-receipt"
    assert variant_packet["sourceReceiptPath"] == "variant-prior.json"
    assert baseline_packet["receiptMetrics"]["aspCommandCount"] == 3
    assert baseline_packet["receiptMetrics"]["metricSource"] == "source-sandtable-receipt"
    assert baseline_packet["sourceReceiptPath"] == "baseline.json"


def test_large_library_variant_batch_all_variant_receipts_unlock_analysis() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    first_run = report_chain["optimizationMatrix"][0]
    assert isinstance(first_run, dict)

    packets = build_large_library_variant_batch(
        report_chain,
        receipt_metrics=_fallback_receipt_metrics(),
        answer_metrics=_fallback_answer_metrics(),
        variant_sandtable_receipts={
            "no-query-seed-prior": _variant_sandtable_receipt(first_run["scenarioId"]),
            "no-package-cohesion": _variant_sandtable_receipt(first_run["scenarioId"]),
            "no-query-clause-coverage": _variant_sandtable_receipt(first_run["scenarioId"]),
            "no-local-evidence": _variant_sandtable_receipt(first_run["scenarioId"]),
            "no-topology-membership": _variant_sandtable_receipt(
                first_run["scenarioId"]
            ),
        },
    )
    analysis = build_large_library_optimization_analysis(report_chain, packets)

    assert analysis["summary"]["status"] == "collecting"
    assert analysis["summary"]["missingVariantRunCount"] == 0
    assert analysis["findings"][0]["kind"] == "fallback-derived-variant-results"

    scoped_chain = dict(report_chain)
    scoped_chain["optimizationMatrix"] = [first_run]
    scoped_chain["optimizationBatch"] = dict(report_chain["optimizationBatch"])
    scoped_chain["optimizationBatch"]["runCount"] = 1
    scoped_chain["optimizationBatch"]["variantRunCount"] = 5
    scoped_packets = build_large_library_variant_batch(
        scoped_chain,
        receipt_metrics=_fallback_receipt_metrics(),
        answer_metrics=_fallback_answer_metrics(),
        variant_sandtable_receipts={
            "no-query-seed-prior": _variant_sandtable_receipt(first_run["scenarioId"]),
            "no-package-cohesion": _variant_sandtable_receipt(first_run["scenarioId"]),
            "no-query-clause-coverage": _variant_sandtable_receipt(first_run["scenarioId"]),
            "no-local-evidence": _variant_sandtable_receipt(first_run["scenarioId"]),
            "no-topology-membership": _variant_sandtable_receipt(
                first_run["scenarioId"]
            ),
        },
    )
    scoped_analysis = build_large_library_optimization_analysis(
        scoped_chain,
        scoped_packets,
    )

    assert scoped_analysis["summary"]["status"] == "analyzed"
    assert scoped_analysis["findings"] == []


def test_large_library_variant_batch_cli_derives_metrics_from_receipt(
    tmp_path: Path,
) -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    first_run = report_chain["optimizationMatrix"][0]
    assert isinstance(first_run, dict)
    report_chain_path = tmp_path / "report-chain.json"
    receipt_path = tmp_path / "sandtable-receipt.json"
    output_path = tmp_path / "variant-batch.json"
    report_chain_path.write_text(json.dumps(report_chain), encoding="utf-8")
    receipt_path.write_text(
        json.dumps(_sandtable_receipt(first_run["scenarioId"])),
        encoding="utf-8",
    )

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-variant-batch",
                str(report_chain_path),
                "--source-sandtable-receipt",
                str(receipt_path),
                "--output",
                str(output_path),
                *_metric_args("--receipt-metric", _fallback_receipt_metrics()),
                *_metric_args("--answer-metric", _fallback_answer_metrics()),
            ]
        )
        == 0
    )

    packets = json.loads(output_path.read_text(encoding="utf-8"))
    assert packets[0]["receiptMetrics"]["aspCommandCount"] == 3
    assert packets[0]["answerMetrics"]["finalAnswerStatus"] == "answered"


def test_large_library_variant_batch_cli_uses_variant_receipt(
    tmp_path: Path,
) -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    first_run = report_chain["optimizationMatrix"][0]
    assert isinstance(first_run, dict)
    report_chain_path = tmp_path / "report-chain.json"
    baseline_path = tmp_path / "baseline-receipt.json"
    variant_path = tmp_path / "variant-receipt.json"
    output_path = tmp_path / "variant-batch.json"
    report_chain_path.write_text(json.dumps(report_chain), encoding="utf-8")
    baseline_path.write_text(
        json.dumps(_sandtable_receipt(first_run["scenarioId"])),
        encoding="utf-8",
    )
    variant_path.write_text(
        json.dumps(_variant_sandtable_receipt(first_run["scenarioId"])),
        encoding="utf-8",
    )

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-variant-batch",
                str(report_chain_path),
                "--source-sandtable-receipt",
                str(baseline_path),
                "--variant-sandtable-receipt",
                f"no-query-seed-prior={variant_path}",
                "--output",
                str(output_path),
                *_metric_args("--receipt-metric", _fallback_receipt_metrics()),
                *_metric_args("--answer-metric", _fallback_answer_metrics()),
            ]
        )
        == 0
    )

    packets = json.loads(output_path.read_text(encoding="utf-8"))
    assert packets[0]["receiptMetrics"]["aspCommandCount"] == 5
    assert packets[0]["sourceReceiptPath"] == str(variant_path.resolve())
    assert packets[1]["receiptMetrics"]["aspCommandCount"] == 3


def _receipt_metrics() -> dict[str, object]:
    return {
        "aspCommandCount": 3,
        "searchCommandCount": 2,
        "queryCommandCount": 1,
        "repeatedCommandCount": 0,
        "commandsToFirstUsefulLocator": 2,
        "frontierFollowRate": 1.0,
        "rawReadFallbackCount": 0,
        "duplicateSelectorCount": 0,
        "sameOwnerScanCount": 0,
        "elapsedMs": 0,
        "stdoutBytes": 0,
        "stderrBytes": 0,
        "queryTopologyMembershipCandidateCount": 0,
        "queryTopologyMembershipCoverageRate": 0.0,
        "queryTopologyMembershipDriftRate": 0.0,
        "queryTopologyMembershipDelta": 0,
    }


def _fallback_receipt_metrics() -> dict[str, object]:
    metrics = _receipt_metrics()
    metrics["aspCommandCount"] = 99
    return metrics


def _answer_metrics() -> dict[str, object]:
    return {
        "finalAnswerStatus": "answered",
        "answerQualityJudgment": 0.9,
        "missingEvidenceCount": 0,
        "wrongOwnerCount": 0,
    }


def _fallback_answer_metrics() -> dict[str, object]:
    metrics = _answer_metrics()
    metrics["answerQualityJudgment"] = 0.25
    return metrics


def _sandtable_receipt(scenario_id: object) -> dict[str, object]:
    return {
        "scenarios": [
            {
                "id": scenario_id,
                "status": "pass",
                "errors": [],
                "flowMetrics": {
                    "commands": 3,
                    "elapsedMs": 123,
                    "stdoutBytes": 456,
                    "stderrBytes": 7,
                },
                "steps": [
                    {
                        "id": "prime",
                        "status": "pass",
                        "command": ["rs-harness", "search", "prime"],
                        "errors": [],
                    },
                    {
                        "id": "intent-query-set",
                        "status": "pass",
                        "command": ["rs-harness", "search", "fzf"],
                        "errors": [],
                    },
                    {
                        "id": "selector-query",
                        "status": "pass",
                        "command": ["rs-harness", "query", "--selector", "src/lib.rs:1"],
                        "errors": [],
                    },
                    {
                        "id": "repeat-query",
                        "status": "pass",
                        "command": ["rs-harness", "query", "--selector", "src/lib.rs:1"],
                        "errors": [],
                    },
                ],
            }
        ]
    }


def _variant_sandtable_receipt(scenario_id: object) -> dict[str, object]:
    receipt = _sandtable_receipt(scenario_id)
    scenario = receipt["scenarios"][0]
    assert isinstance(scenario, dict)
    scenario["flowMetrics"] = {
        "commands": 5,
        "elapsedMs": 777,
        "stdoutBytes": 888,
        "stderrBytes": 9,
    }
    steps = scenario["steps"]
    assert isinstance(steps, list)
    steps.append(
        {
            "id": "variant-owner",
            "status": "pass",
            "command": ["rs-harness", "search", "owner"],
            "errors": [],
        }
    )
    return receipt


def _metric_args(flag: str, metrics: dict[str, object]) -> list[str]:
    args = []
    for key, value in metrics.items():
        args.extend([flag, f"{key}={value}"])
    return args


def _validate_variant_schema(packet: dict[str, object]) -> None:
    schema = json.loads(
        (
            _ROOT
            / "schemas"
            / (
                "semantic-sandtable-large-library-optimization-variant-result."
                "v1.schema.json"
            )
        ).read_text(encoding="utf-8")
    )
    Draft202012Validator(schema).validate(packet)
