from __future__ import annotations

import json
import sys
from copy import deepcopy
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

_TESTS_ROOT = Path(__file__).resolve().parent
if str(_TESTS_ROOT) not in sys.path:
    sys.path.insert(0, str(_TESTS_ROOT))

from schema_validation import schema_validator_for  # noqa: E402

_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = (
    _REPO_ROOT / "schemas" / "semantic-fact-frontier-benchmark-report.v1.schema.json"
)
_FIXTURES_PATH = (
    _REPO_ROOT / "schemas" / "semantic-fact-frontier-benchmark-report.fixtures.v1.json"
)
_RECEIPT_FIXTURES_PATH = (
    _REPO_ROOT / "schemas" / "semantic-fact-frontier-receipt.fixtures.v1.json"
)


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _validator() -> Draft202012Validator:
    return schema_validator_for(_SCHEMA_PATH)


def _reports() -> list[dict[str, Any]]:
    return _load_json(_FIXTURES_PATH)["reports"]


def _receipt_fixtures_by_id() -> dict[str, dict[str, Any]]:
    catalog = _load_json(_RECEIPT_FIXTURES_PATH)
    return {fixture["fixtureId"]: fixture for fixture in catalog["fixtures"]}


def _ratio(numerator: int, denominator: int) -> float:
    if denominator == 0:
        return 0.0
    return numerator / denominator


def _derived_context_metrics(scenario: dict[str, Any], receipt: dict[str, Any]) -> dict[str, Any]:
    gold_context = scenario["goldContext"]
    gold_frontier = set(gold_context["goldFrontierNodeIds"])
    gold_selectors = set(gold_context["goldSelectors"])
    returned_ids = {item["nodeId"] for item in receipt["frontierReturned"]}
    followed_ids = {item["nodeId"] for item in receipt["frontierFollowed"]}
    read_selectors = {read["selector"] for read in receipt["codeActuallyRead"]}
    emitted_count = len(receipt["frontierReturned"])
    gold_frontier_count = len(gold_frontier)
    gold_frontier_hit_count = len(gold_frontier & returned_ids)
    followed_gold_frontier_count = len(gold_frontier & followed_ids)
    gold_selector_read_count = len(gold_selectors & read_selectors)
    test_command = receipt.get("testCommand") or {}
    test_result = receipt.get("testResult") or {}
    gold_test_action = gold_context["goldTestAction"]
    test_selection_precision = (
        1.0
        if test_command.get("argv") == gold_test_action["argv"]
        and test_result.get("status") == gold_test_action["expectedStatus"]
        else 0.0
    )

    return {
        "goldFrontierCount": gold_frontier_count,
        "emittedFrontierCount": emitted_count,
        "goldFrontierHitCount": gold_frontier_hit_count,
        "followedGoldFrontierCount": followed_gold_frontier_count,
        "goldSelectorReadCount": gold_selector_read_count,
        "contextPrecision": _ratio(gold_frontier_hit_count, emitted_count),
        "contextRecall": _ratio(gold_frontier_hit_count, gold_frontier_count),
        "contextUtilization": _ratio(followed_gold_frontier_count, emitted_count),
        "exactCodeSuccess": gold_selector_read_count > 0,
        "testSelectionPrecision": test_selection_precision,
    }


def _assert_context_metrics_match(actual: dict[str, Any], expected: dict[str, Any]) -> None:
    for key, value in expected.items():
        if isinstance(value, float):
            assert abs(actual[key] - value) < 1e-12, key
        else:
            assert actual[key] == value, key


def test_frontier_benchmark_report_fixtures_match_schema() -> None:
    validator = _validator()

    for report in _reports():
        errors = sorted(validator.iter_errors(report), key=lambda error: error.path)

        assert not errors, [error.message for error in errors]


def test_frontier_benchmark_report_scenarios_match_receipt_fixtures() -> None:
    receipt_fixtures = _receipt_fixtures_by_id()

    for report in _reports():
        assert (
            report["sourceReceiptFixtureCatalog"]
            == "schemas/semantic-fact-frontier-receipt.fixtures.v1.json"
        )
        assert (
            report["sourceBenchmarkFixture"]
            == "sandtables/fixtures/asp/graph-turbo-owner-query.json"
        )

        for scenario in report["scenarios"]:
            receipt_fixture = receipt_fixtures[scenario["receiptFixtureId"]]
            receipt = receipt_fixture["receipt"]

            assert scenario["receiptId"] == receipt["receiptId"]
            assert scenario["receiptMetrics"] == receipt["metrics"]


def test_frontier_benchmark_report_context_metrics_are_derived_from_gold_context() -> None:
    receipt_fixtures = _receipt_fixtures_by_id()

    for report in _reports():
        for scenario in report["scenarios"]:
            receipt_fixture = receipt_fixtures[scenario["receiptFixtureId"]]
            receipt = receipt_fixture["receipt"]

            assert scenario["contextMetrics"]["emittedFrontierCount"] == scenario[
                "receiptMetrics"
            ]["frontierReturnedCount"]
            _assert_context_metrics_match(
                scenario["contextMetrics"],
                _derived_context_metrics(scenario, receipt),
            )


def test_frontier_benchmark_report_summary_is_derived_from_scenarios() -> None:
    receipt_fixtures = _receipt_fixtures_by_id()

    for report in _reports():
        scenarios = report["scenarios"]
        summary = report["summary"]

        assert summary["scenarioCount"] == len(scenarios)
        assert summary["receiptFixtureCount"] == len(receipt_fixtures)
        assert summary["minFrontierFollowRate"] == min(
            scenario["receiptMetrics"]["frontierFollowRate"] for scenario in scenarios
        )
        assert summary["maxRawReadFallbackCount"] == max(
            scenario["receiptMetrics"]["rawReadFallbackCount"] for scenario in scenarios
        )
        assert summary["allRelationChannelsVisible"] is all(
            scenario["receiptMetrics"]["relationChannelCount"] > 0
            for scenario in scenarios
        )
        assert summary["runtimeCaptureScenarioCount"] == sum(
            scenario["benchmarkReadiness"]["hasRuntimeCapture"]
            for scenario in scenarios
        )
        assert summary["calibrationReadyScenarioCount"] == sum(
            scenario["benchmarkReadiness"]["readyForWeightCalibration"]
            for scenario in scenarios
        )


def test_frontier_benchmark_report_requires_followed_runtime_use_for_calibration() -> None:
    report = _reports()[0]
    scenarios = {scenario["scenarioId"]: scenario for scenario in report["scenarios"]}

    asp_runtime = scenarios["asp-runtime-frontier-only"]
    followed_runtime = scenarios["asp-runtime-followed-read-test"]

    assert asp_runtime["benchmarkReadiness"]["hasRuntimeCapture"] is True
    assert asp_runtime["benchmarkReadiness"]["hasFollowedFrontier"] is False
    assert asp_runtime["benchmarkReadiness"]["hasValidationCommand"] is False
    assert asp_runtime["benchmarkReadiness"]["readyForWeightCalibration"] is False
    assert followed_runtime["benchmarkReadiness"]["hasRuntimeCapture"] is True
    assert followed_runtime["benchmarkReadiness"]["hasFollowedFrontier"] is True
    assert followed_runtime["benchmarkReadiness"]["hasValidationCommand"] is True
    assert followed_runtime["benchmarkReadiness"]["readyForWeightCalibration"] is True
    assert followed_runtime["contextMetrics"]["exactCodeSuccess"] is True
    assert followed_runtime["contextMetrics"]["testSelectionPrecision"] == 1.0
    assert followed_runtime["contextMetrics"]["contextUtilization"] > 0
    assert report["summary"]["calibrationReadyScenarioCount"] == 1


def test_frontier_benchmark_report_rejects_unknown_capture_kind() -> None:
    report = deepcopy(_reports()[0])
    report["scenarios"][0]["captureKind"] = "obsolete-frontier-kind"

    errors = sorted(_validator().iter_errors(report), key=lambda error: error.path)

    assert any("is not one of" in error.message for error in errors)


def test_frontier_benchmark_report_requires_benchmark_metrics() -> None:
    report = deepcopy(_reports()[0])
    report["scenarios"][0].pop("benchmarkMetrics")

    errors = sorted(_validator().iter_errors(report), key=lambda error: error.path)

    assert any("'benchmarkMetrics' is a required property" in error.message for error in errors)


def test_frontier_benchmark_report_requires_context_metrics() -> None:
    report = deepcopy(_reports()[0])
    report["scenarios"][0].pop("contextMetrics")

    errors = sorted(_validator().iter_errors(report), key=lambda error: error.path)

    assert any("'contextMetrics' is a required property" in error.message for error in errors)
