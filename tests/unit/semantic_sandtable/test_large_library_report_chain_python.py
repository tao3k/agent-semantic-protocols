"""Python large-library report-chain readiness tests."""

from __future__ import annotations

import json
from pathlib import Path

from tools.semantic_sandtable.large_library_report_chain import (
    build_large_library_report_chain,
)

from .test_large_library_report_chain import (
    _matrix_targets_query_first_stage,
    _scenario,
    _scenario_command,
    _validate_schema,
)


def test_large_library_report_chain_can_pass_with_python_fixture(
    tmp_path: Path,
) -> None:
    path = tmp_path / "python.json"
    path.write_text(json.dumps(_scenario("python")), encoding="utf-8")

    report = build_large_library_report_chain(tmp_path, [path], languages=("python",))

    _validate_schema(report)
    assert report["optimizationGate"]["status"] == "pass"
    assert report["rollup"]["readyLanguageCount"] == 1
    assert report["rollup"]["optimizationRunCount"] == 3
    assert report["rollup"]["optimizationVariantRunCount"] == 15
    assert report["optimizationBatch"]["readyToCollectReceipts"] is True
    assert report["benchmarkData"] == {
        "scenarioCount": 1,
        "searchCommandCount": 1,
        "uniqueSearchCommandCount": 1,
        "optimizationRunCount": 3,
        "optimizationVariantRunCount": 15,
        "ablationVariantCount": 5,
        "coveredSearchMethods": ["search/lexical"],
        "coveredSearchQueries": ["feature", "owner"],
        "byLanguage": [
            {
                "language": "python",
                "scenarioCount": 1,
                "searchCommandCount": 1,
                "uniqueSearchCommandCount": 1,
                "optimizationRunCount": 3,
                "optimizationVariantRunCount": 15,
                "coveredSearchMethods": ["search/lexical"],
                "coveredSearchQueries": ["feature", "owner"],
            }
        ],
    }
    command = _scenario_command("python")
    assert report["searchCommandSet"] == [
        {
            "commandId": " ".join(command),
            "language": "python",
            "method": "search/lexical",
            "view": "lexical",
            "queries": ["feature", "owner"],
            "command": command,
            "scenarioIds": ["python.multi-depth"],
        }
    ]
    assert report["findings"] == []
    assert report["languages"][0]["language"] == "python"
    assert report["languages"][0]["depthBucketCounts"] == {
        "deep": 1,
        "medium": 1,
        "strict": 1,
    }
    assert _matrix_targets_query_first_stage(report)
