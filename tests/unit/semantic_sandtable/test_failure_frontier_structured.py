# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Structured failure-frontier comparison tests."""

from __future__ import annotations

from tools.semantic_sandtable.failure_frontier_eval import (
    FailureFrontierThresholds,
    compare_failure_frontier_receipts,
)


_TEST_BLOCK = (
    "crates/agent-semantic-client/tests/unit/cache_cli/writeback/search.rs:40-72"
)


def test_compare_receipts_uses_structured_failure_frontier() -> None:
    baseline = _receipt(
        "rust.cache-replay-baseline",
        [
            _direct_read(f"window-{index}", f"src/cache.rs:{index}-{index + 1}")
            for index in range(4)
        ],
        stdout_bytes=400,
    )
    candidate = _receipt(
        "rust.cache-replay-structured-frontier",
        [_direct_read("test-a", _TEST_BLOCK, stdout_bytes=100)],
        stdout_bytes=100,
        policy_receipts=[_structured_frontier_receipt(_TEST_BLOCK)],
    )

    comparison = compare_failure_frontier_receipts(
        baseline,
        candidate,
        expected_hot_blocks=[],
        thresholds=FailureFrontierThresholds(
            min_command_reduction=0.5,
            max_direct_source_read_code=1,
        ),
    )

    assert comparison["status"] == "pass"
    assert comparison["frontier"]["declaredHotBlocks"] == [_TEST_BLOCK]
    assert comparison["frontier"]["expectedHotBlocks"] == [_TEST_BLOCK]
    assert comparison["frontier"]["coveredHotBlocks"] == [_TEST_BLOCK]
    assert comparison["frontier"]["declaredFailureFrontier"] == [
        {
            "rule": "RUST-PROJ-R003",
            "severity": "error",
            "path": "src/lib.rs",
            "line": 1,
            "column": 1,
            "message": "Rust project check failed",
            "summary": "cargo check reported an owner failure",
            "repair": "repair the Rust owner",
            "hotBlockSelector": _TEST_BLOCK,
            "hotBlockReason": "blocking-finding",
            "nextAction": "direct-source-read",
            "nextSelector": _TEST_BLOCK,
            "nextRoot": ".",
        }
    ]


def _receipt(
    scenario_id: str,
    commands: list[dict[str, object]],
    *,
    stdout_bytes: int,
    policy_receipts: list[dict[str, object]] | None = None,
) -> dict[str, object]:
    for command in commands:
        metrics = command["metrics"]
        assert isinstance(metrics, dict)
        metrics.setdefault("stdoutBytes", stdout_bytes)
    return {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
        "schemaVersion": "1",
        "scenarioId": scenario_id,
        "language": "rust",
        "project": {"name": "agent-semantic-protocols", "source": "fixture"},
        "intent": "Compare baseline source-window scan with failure-frontier flow.",
        "editBoundary": "before-edit",
        "commands": commands,
        "policyReceipts": policy_receipts or [],
        "summary": {
            "commandCount": len(commands),
            "stdoutBytes": sum(
                _metric_int(command, "stdoutBytes") for command in commands
            ),
            "stderrBytes": 0,
            "elapsedMs": sum(_metric_int(command, "elapsedMs") for command in commands),
        },
    }


def _structured_frontier_receipt(selector: str) -> dict[str, object]:
    return {
        "receiptId": "rust.policy.failure-frontier",
        "authority": "asp-rust-build-dependency-api",
        "failureFrontier": [
            {
                "rule": "RUST-PROJ-R003",
                "severity": "error",
                "path": "src/lib.rs",
                "line": 1,
                "column": 1,
                "message": "Rust project check failed",
                "summary": "cargo check reported an owner failure",
                "repair": "repair the Rust owner",
                "hotBlockSelector": selector,
                "hotBlockReason": "blocking-finding",
                "nextAction": "direct-source-read",
                "nextSelector": selector,
                "nextRoot": ".",
            }
        ],
    }


def _direct_read(
    command_id: str,
    selector: str,
    *,
    stdout_bytes: int = 700,
) -> dict[str, object]:
    return {
        "id": command_id,
        "kind": "other",
        "argv": [
            "asp",
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            selector,
            "--code",
            ".",
        ],
        "outputMode": "compact",
        "metrics": {
            "elapsedMs": 3,
            "stdoutBytes": stdout_bytes,
            "stderrBytes": 0,
        },
    }


def _metric_int(command: dict[str, object], field: str) -> int:
    metrics = command["metrics"]
    assert isinstance(metrics, dict)
    value = metrics[field]
    assert isinstance(value, int)
    return value
