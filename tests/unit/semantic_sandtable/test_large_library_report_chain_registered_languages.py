"""Registered-language large-library report-chain performance gates."""

from __future__ import annotations

from pathlib import Path

from tools.semantic_sandtable.large_library_report_chain import (
    build_large_library_report_chain,
)

from .test_large_library_report_chain import (
    _assert_no_legacy_search_commands,
    _finding_kinds,
    _language_benchmark_counts,
    _matrix_depth_counts,
    _matrix_targets_query_first_stage,
    _validate_schema,
)

_ROOT = Path(__file__).resolve().parents[3]


def test_large_library_report_chain_unblocks_tuning_with_registered_language_depths() -> None:
    report = build_large_library_report_chain(_ROOT)

    _validate_schema(report)
    by_language = {entry["language"]: entry for entry in report["languages"]}

    assert report["optimizationGate"]["status"] == "pass"
    assert report["findings"] == []
    assert report["rollup"] == {
        "languageCount": 4,
        "scenarioCount": 23,
        "libraryCount": 14,
        "deepQuestionCount": 26,
        "readyLanguageCount": 4,
        "optimizationRunCount": 26,
        "optimizationVariantRunCount": 130,
        "findingCount": 0,
        "aspBinaryFreshnessRiskCommandCount": 0,
        "aspBinaryFreshnessRiskScenarioCount": 0,
    }
    assert report["optimizationBatch"]["runCount"] == 26
    assert report["optimizationBatch"]["ablationVariantCount"] == 5
    assert report["optimizationBatch"]["variantRunCount"] == 130
    assert report["benchmarkData"]["scenarioCount"] == 23
    assert report["benchmarkData"]["searchCommandCount"] == 56
    assert report["benchmarkData"]["uniqueSearchCommandCount"] == 47
    assert report["benchmarkData"]["optimizationRunCount"] == 26
    assert report["benchmarkData"]["optimizationVariantRunCount"] == 130
    assert report["benchmarkData"]["ablationVariantCount"] == 5
    assert report["benchmarkData"]["coveredSearchMethods"] == [
        "search/deps",
        "search/ingest",
        "search/lexical",
        "search/owner",
        "search/pipe",
        "search/prime",
        "search/tests",
        "search/workspace",
    ]
    assert len(report["searchCommandSet"]) == 47
    _assert_no_legacy_search_commands(report)
    assert "no-local-evidence" in report["optimizationBatch"]["ablationVariants"]
    assert report["optimizationBatch"]["aggregationAxes"] == [
        "language",
        "package",
        "depthBucket",
        "ablationVariant",
    ]
    assert "frontierFollowRate" in report["optimizationBatch"][
        "requiredReceiptMetrics"
    ]
    assert "answerQualityJudgment" in report["optimizationBatch"][
        "requiredAnswerMetrics"
    ]
    assert len(report["optimizationMatrix"]) == 26
    assert _matrix_depth_counts(report) == {
        "julia": {"deep": 1, "medium": 1, "strict": 1},
        "python": {"deep": 1, "medium": 1, "strict": 1},
        "rust": {"deep": 10, "medium": 1, "strict": 2},
        "typescript": {"deep": 4, "medium": 1, "strict": 2},
    }
    assert _matrix_targets_query_first_stage(report)
    for language in ("julia", "python"):
        assert by_language[language]["deepQuestionCount"] == 3
        assert by_language[language]["depthBucketCounts"] == {
            "deep": 1,
            "medium": 1,
            "strict": 1,
        }
        assert _finding_kinds(by_language[language]) == set()
    assert by_language["rust"]["deepQuestionCount"] >= 12
    assert _finding_kinds(by_language["rust"]) == set()
    assert by_language["typescript"]["deepQuestionCount"] == 7
    assert _finding_kinds(by_language["typescript"]) == set()


def test_large_library_report_chain_benchmarks_all_registered_search_languages() -> None:
    report = build_large_library_report_chain(
        _ROOT,
        languages=("julia", "python", "rust", "typescript"),
    )

    _validate_schema(report)
    benchmark = report["benchmarkData"]

    assert report["rollup"]["languageCount"] == 4
    assert benchmark["scenarioCount"] == 23
    assert benchmark["searchCommandCount"] == 56
    assert benchmark["uniqueSearchCommandCount"] == 47
    assert benchmark["optimizationRunCount"] == 26
    assert benchmark["optimizationVariantRunCount"] == 130
    assert benchmark["coveredSearchMethods"] == [
        "search/deps",
        "search/ingest",
        "search/lexical",
        "search/owner",
        "search/pipe",
        "search/prime",
        "search/tests",
        "search/workspace",
    ]
    assert len(report["searchCommandSet"]) == 47
    assert _language_benchmark_counts(report) == {
        "julia": (4, 9, 7, 3, 15),
        "python": (5, 11, 10, 3, 15),
        "rust": (5, 9, 7, 13, 65),
        "typescript": (9, 27, 23, 7, 35),
    }
    assert all(
        "search/lexical" in entry["coveredSearchMethods"]
        for entry in benchmark["byLanguage"]
    )
    _assert_no_legacy_search_commands(report)
