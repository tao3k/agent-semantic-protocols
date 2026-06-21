"""Large-library optimization variant result tests."""

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
from tools.semantic_sandtable.large_library_variant_result import (
    build_large_library_variant_result,
)

_ROOT = Path(__file__).resolve().parents[3]


def test_large_library_variant_result_validates_schema() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    variant_run_id = _first_variant_run_id(report_chain)

    packet = build_large_library_variant_result(
        report_chain,
        variant_run_id=variant_run_id,
        receipt_metrics=_receipt_metrics(),
        answer_metrics=_answer_metrics(),
        source_receipt_path="receipts/example.json",
    )

    _validate_schema(packet)
    assert packet["variantRunId"] == variant_run_id
    assert packet["packetKind"] == "large-library-optimization-variant-result"
    assert packet["sourceReceiptPath"] == "receipts/example.json"


def test_large_library_variant_result_feeds_analysis_observed_count() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    packet = build_large_library_variant_result(
        report_chain,
        variant_run_id=_first_variant_run_id(report_chain),
        receipt_metrics=_receipt_metrics(),
        answer_metrics=_answer_metrics(),
    )

    analysis = build_large_library_optimization_analysis(report_chain, [packet])

    assert analysis["summary"]["observedVariantRunCount"] == 1
    assert analysis["summary"]["missingVariantRunCount"] == 99
    assert packet["variantRunId"] in analysis["collectionManifest"][
        "observedVariantRunIds"
    ]


def test_large_library_variant_result_cli_emits_json(
    tmp_path: Path,
    capsys,
) -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    report_chain_path = tmp_path / "report-chain.json"
    report_chain_path.write_text(json.dumps(report_chain), encoding="utf-8")

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-variant-result",
                str(report_chain_path),
                "--variant-run-id",
                _first_variant_run_id(report_chain),
                "--json",
                *_metric_args("--receipt-metric", _receipt_metrics()),
                *_metric_args("--answer-metric", _answer_metrics()),
            ]
        )
        == 0
    )

    packet = json.loads(capsys.readouterr().out)
    _validate_schema(packet)
    assert packet["receiptMetrics"]["frontierFollowRate"] == 1.0
    assert packet["answerMetrics"]["finalAnswerStatus"] == "answered"


def test_large_library_variant_result_cli_writes_output(
    tmp_path: Path,
    capsys,
) -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    report_chain_path = tmp_path / "report-chain.json"
    output_path = tmp_path / "variant-result.json"
    report_chain_path.write_text(json.dumps(report_chain), encoding="utf-8")

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-variant-result",
                str(report_chain_path),
                "--variant-run-id",
                _first_variant_run_id(report_chain),
                "--output",
                str(output_path),
                *_metric_args("--receipt-metric", _receipt_metrics()),
                *_metric_args("--answer-metric", _answer_metrics()),
            ]
        )
        == 0
    )

    assert capsys.readouterr().out == ""
    packet = json.loads(output_path.read_text(encoding="utf-8"))
    _validate_schema(packet)
    assert packet["variantRunId"] == _first_variant_run_id(report_chain)


def test_large_library_variant_result_cli_requires_metrics(
    tmp_path: Path,
) -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    report_chain_path = tmp_path / "report-chain.json"
    report_chain_path.write_text(json.dumps(report_chain), encoding="utf-8")

    try:
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-variant-result",
                str(report_chain_path),
                "--variant-run-id",
                _first_variant_run_id(report_chain),
            ]
        )
    except SystemExit as error:
        assert "receiptMetrics missing required metrics" in str(error)
    else:
        raise AssertionError("expected missing receipt metrics to fail")


def _first_variant_run_id(report_chain: dict[str, object]) -> str:
    run = report_chain["optimizationMatrix"][0]
    assert isinstance(run, dict)
    variant = run["ablationVariants"][0]
    return f"{run['runId']}:{variant}"


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
        "elapsedMs": 100,
        "stdoutBytes": 200,
        "stderrBytes": 0,
    }


def _answer_metrics() -> dict[str, object]:
    return {
        "finalAnswerStatus": "answered",
        "answerQualityJudgment": 0.9,
        "missingEvidenceCount": 0,
        "wrongOwnerCount": 0,
    }


def _metric_args(flag: str, metrics: dict[str, object]) -> list[str]:
    args = []
    for key, value in metrics.items():
        args.extend([flag, f"{key}={value}"])
    return args


def _validate_schema(packet: dict[str, object]) -> None:
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
