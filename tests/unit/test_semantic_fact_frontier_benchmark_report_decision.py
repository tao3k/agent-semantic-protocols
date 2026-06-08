from __future__ import annotations

import json
from pathlib import Path
from typing import Any

_REPO_ROOT = Path(__file__).resolve().parents[2]
_FIXTURES_PATH = (
    _REPO_ROOT / "schemas" / "semantic-fact-frontier-benchmark-report.fixtures.v1.json"
)


def _report() -> dict[str, Any]:
    catalog = json.loads(_FIXTURES_PATH.read_text(encoding="utf-8"))
    return catalog["reports"][0]


def test_relation_weight_changes_wait_for_new_failing_runtime_evidence() -> None:
    report = _report()
    scenarios = report["scenarios"]
    ready_scenarios = [
        scenario
        for scenario in scenarios
        if scenario["benchmarkReadiness"]["readyForWeightCalibration"]
    ]
    weights_runtime = {
        scenario["scenarioId"]: scenario for scenario in scenarios
    }["asp-runtime-relation-weights-followed-read-test"]

    assert len(ready_scenarios) == 3
    assert all(
        scenario["contextMetrics"]["contextRecall"] == 1.0
        for scenario in ready_scenarios
    )
    assert weights_runtime["contextMetrics"]["goldFrontierBestRank"] == 3
    assert weights_runtime["contextMetrics"]["goldSelectorActionRank"] == 1
    assert report["summary"]["weightCalibrationDecision"] == {
        "status": "deferred-until-new-failing-receipt",
        "reason": (
            "hot-companion ranking now preserves the relation-weight gold "
            "frontier and selector action"
        ),
        "requiredEvidence": (
            "new calibration-ready runtime receipt with contextRecall below 1.0 "
            "or the gold selector missing from frontierActions"
        ),
    }
    assert (
        report["summary"]["nextAction"]
        == "collect new failing calibration-ready evidence before changing relation weights"
    )
