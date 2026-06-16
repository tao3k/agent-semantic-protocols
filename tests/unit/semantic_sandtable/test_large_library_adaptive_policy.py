"""Large-library adaptive graph-turbo policy tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.large_library_adaptive_policy import (
    build_large_library_adaptive_policy,
)
from tools.semantic_sandtable.large_library_optimization_analysis import (
    build_large_library_optimization_analysis,
)
from tools.semantic_sandtable.large_library_report_chain import (
    build_large_library_report_chain,
)

from .test_large_library_optimization_analysis import _variant_results

_ROOT = Path(__file__).resolve().parents[3]


def test_large_library_adaptive_policy_uses_analysis_recommendations() -> None:
    analysis = _analysis()

    packet = build_large_library_adaptive_policy(analysis)

    _validate_schema(packet)
    assert packet["status"] == "ready"
    assert packet["targetGraphPhase"] == "query-first-stage"
    default_policy = packet["defaultPolicy"]
    assert isinstance(default_policy, dict)
    assert default_policy["ablationVariant"] == "no-package-cohesion"
    assert default_policy["queryAdjustmentPolicy"] == {"packageCohesion": False}
    assert default_policy["averageAnswerQuality"] == 0.9
    assert default_policy["averageElapsedMs"] == 10.0
    assert default_policy["averageStdoutBytes"] == 1000.0
    assert default_policy["resultCount"] == 20
    assert packet["bucketPolicies"]
    first_bucket = packet["bucketPolicies"][0]
    assert first_bucket["evidence"]["granularity"] == "scenario-receipt"
    assert first_bucket["evidence"]["scenarioIds"]
    assert packet["guardrails"]["bucketGranularity"] == "scenario-receipt"
    assert (
        packet["guardrails"]["nextValidationGranularity"]
        == "per-question-live-agent-receipt"
    )
    validation_plan = packet["validationPlan"]
    assert validation_plan["targetGranularity"] == "per-question-live-agent-receipt"
    assert validation_plan["runCount"] == len(validation_plan["runs"])
    assert validation_plan["runs"]
    first_run = validation_plan["runs"][0]
    assert first_run["scenarioPath"].startswith("sandtables/")
    assert first_run["questionId"]
    assert first_run["env"] == {
        "ASP_GRAPH_TURBO_ABLATION_VARIANT": first_run["ablationVariant"]
    }
    assert first_run["expectedReceiptGranularity"] == "per-question-live-agent"
    assert packet["runtimeBridge"]["envVar"] == "ASP_GRAPH_TURBO_ABLATION_VARIANT"


def test_large_library_adaptive_policy_cli_writes_output(tmp_path: Path) -> None:
    analysis_path = tmp_path / "analysis.json"
    output_path = tmp_path / "adaptive-policy.json"
    analysis_path.write_text(json.dumps(_analysis()), encoding="utf-8")

    assert (
        main(
            [
                "--repo-root",
                str(_ROOT),
                "--large-library-adaptive-policy",
                str(analysis_path),
                "--output",
                str(output_path),
            ]
        )
        == 0
    )

    packet = json.loads(output_path.read_text(encoding="utf-8"))
    _validate_schema(packet)
    assert packet["status"] == "ready"


def _analysis() -> dict[str, object]:
    report_chain = build_large_library_report_chain(_ROOT)
    return build_large_library_optimization_analysis(
        report_chain,
        _variant_results(report_chain),
    )


def _validate_schema(packet: dict[str, object]) -> None:
    schema = json.loads(
        (
            _ROOT
            / "schemas"
            / "semantic-graph-turbo-adaptive-query-policy.v1.schema.json"
        ).read_text(encoding="utf-8")
    )
    Draft202012Validator(schema).validate(packet)
