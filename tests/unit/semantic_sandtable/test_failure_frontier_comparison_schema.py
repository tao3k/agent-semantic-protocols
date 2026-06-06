"""Schema tests for failure-frontier sandtable comparison packets."""

from __future__ import annotations

from pathlib import Path

from tools.semantic_sandtable.failure_frontier_eval import (
    FailureFrontierThresholds,
    compare_failure_frontier_receipt_paths,
)
from unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[3]
_SCHEMA_PATH = (
    _REPO_ROOT
    / "schemas"
    / "semantic-sandtable-failure-frontier-comparison.v1.schema.json"
)
_BASELINE_RECEIPT = (
    _REPO_ROOT
    / "sandtables"
    / "fixtures"
    / "asp"
    / "failure-frontier-baseline-receipt.json"
)
_CANDIDATE_RECEIPT = (
    _REPO_ROOT
    / "sandtables"
    / "fixtures"
    / "asp"
    / "failure-frontier-candidate-receipt.json"
)
_HOT_BLOCKS = [
    "crates/agent-semantic-client/tests/unit/cache_cli/writeback/search.rs:40-72",
    "crates/agent-semantic-client/src/cache_cli/writeback.rs:220-260",
    "crates/agent-semantic-client/src/cache_replay/artifact.rs:88-132",
    "crates/agent-semantic-client/src/cache_cli/probe.rs:140-205",
]


def test_failure_frontier_comparison_packet_is_schema_valid() -> None:
    packet = compare_failure_frontier_receipt_paths(
        _REPO_ROOT,
        _BASELINE_RECEIPT,
        _CANDIDATE_RECEIPT,
        expected_hot_blocks=_HOT_BLOCKS,
        thresholds=FailureFrontierThresholds(),
    )
    errors = list(schema_validator_for(_SCHEMA_PATH).iter_errors(packet))

    assert errors == []
    assert packet["schemaId"] == (
        "agent.semantic-protocols.semantic-sandtable-failure-frontier-comparison"
    )
    assert packet["status"] == "pass"
    assert packet["delta"]["commandReductionRatio"] == 0.5
    assert packet["frontier"]["coverageRatio"] == 1.0
    assert packet["thresholds"]["maxSameFileWindowFanout"] == 0


def test_failure_frontier_comparison_schema_rejects_unknown_metric() -> None:
    packet = compare_failure_frontier_receipt_paths(
        _REPO_ROOT,
        _BASELINE_RECEIPT,
        _CANDIDATE_RECEIPT,
        expected_hot_blocks=_HOT_BLOCKS,
        thresholds=FailureFrontierThresholds(),
    )
    candidate = packet["candidate"]
    assert isinstance(candidate, dict)
    candidate["rawSourceWindowCount"] = 3

    errors = list(schema_validator_for(_SCHEMA_PATH).iter_errors(packet))

    assert any(
        "Additional properties are not allowed" in error.message for error in errors
    )
