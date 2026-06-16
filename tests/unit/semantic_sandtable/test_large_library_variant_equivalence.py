"""Large-library variant receipt equivalence tests."""

from __future__ import annotations

from pathlib import Path

from tools.semantic_sandtable.large_library_optimization_analysis import (
    build_large_library_optimization_analysis,
)
from tools.semantic_sandtable.large_library_report_chain import (
    build_large_library_report_chain,
)
from tools.semantic_sandtable.large_library_variant_batch import (
    build_large_library_variant_batch,
)

from .test_large_library_variant_batch import (
    _fallback_answer_metrics,
    _fallback_receipt_metrics,
    _sandtable_receipt,
)

_ROOT = Path(__file__).resolve().parents[3]


def test_large_library_variant_batch_marks_source_equivalent_variant_receipts() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    first_run = report_chain["optimizationMatrix"][0]
    assert isinstance(first_run, dict)
    baseline_receipt = _sandtable_receipt(first_run["scenarioId"])

    packets = build_large_library_variant_batch(
        report_chain,
        receipt_metrics=_fallback_receipt_metrics(),
        answer_metrics=_fallback_answer_metrics(),
        sandtable_receipt=baseline_receipt,
        variant_sandtable_receipts={
            "no-query-seed-prior": baseline_receipt,
        },
    )

    assert packets[0]["ablationVariant"] == "no-query-seed-prior"
    assert (
        packets[0]["receiptMetrics"]["metricSource"]
        == "source-equivalent-variant-receipt"
    )
    analysis = build_large_library_optimization_analysis(report_chain, packets)
    assert analysis["summary"]["status"] == "collecting"
    assert "source-equivalent-variant-results" in {
        item["kind"] for item in analysis["findings"] if isinstance(item, dict)
    }
    assert "source-equivalent" in {
        item["collectionStatus"]
        for item in analysis["collectionManifest"]["runsNeedingVariantReceipt"]
        if isinstance(item, dict)
    }


def test_large_library_variant_batch_ignores_volatile_timing_for_equivalence() -> None:
    report_chain = build_large_library_report_chain(_ROOT)
    first_run = report_chain["optimizationMatrix"][0]
    assert isinstance(first_run, dict)
    baseline_receipt = _sandtable_receipt(first_run["scenarioId"])
    timing_only_receipt = _sandtable_receipt(first_run["scenarioId"])
    scenario = timing_only_receipt["scenarios"][0]
    assert isinstance(scenario, dict)
    flow_metrics = scenario["flowMetrics"]
    assert isinstance(flow_metrics, dict)
    flow_metrics["elapsedMs"] = 999
    first_step = scenario["steps"][0]
    assert isinstance(first_step, dict)
    first_step["elapsedMs"] = 888

    packets = build_large_library_variant_batch(
        report_chain,
        receipt_metrics=_fallback_receipt_metrics(),
        answer_metrics=_fallback_answer_metrics(),
        sandtable_receipt=baseline_receipt,
        variant_sandtable_receipts={
            "no-query-seed-prior": timing_only_receipt,
        },
    )

    assert (
        packets[0]["receiptMetrics"]["metricSource"]
        == "source-equivalent-variant-receipt"
    )
