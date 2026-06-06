"""Build sandtable receipts from recorded command traces."""

from __future__ import annotations

import json
from pathlib import Path

from tools.semantic_sandtable.receipts import validate_receipt_consistency
from tools.semantic_sandtable.trace_receipts import (
    TraceReceiptConfig,
    build_receipt_from_trace_path,
)

from .trace_receipt_fixtures import REPLAY_BLOCK, TEST_BLOCK, WRITEBACK_BLOCK


def test_build_receipt_from_jsonl_and_text_trace(tmp_path: Path) -> None:
    trace_path = tmp_path / "trace.jsonl"
    trace_path.write_text(
        "\n".join(
            [
                json.dumps(_frontier_event()),
                json.dumps(_query_event()),
                (
                    "$ asp rust query --from-hook direct-source-read "
                    f"--selector {REPLAY_BLOCK} --code ."
                ),
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    receipt = build_receipt_from_trace_path(
        trace_path,
        config=TraceReceiptConfig(
            scenario_id="rust.trace-builder",
            language="rust",
            project_name="agent-semantic-protocols",
            project_source="fixture",
            intent="Build receipt from a mixed trace fixture.",
        ),
    )

    validate_receipt_consistency(receipt)
    assert receipt["schemaId"] == "agent.semantic-protocols.semantic-sandtable-receipt"
    assert receipt["scenarioId"] == "rust.trace-builder"
    assert receipt["summary"] == _summary()
    commands = receipt["commands"]
    assert isinstance(commands, list)
    assert commands[0]["kind"] == "check"
    assert commands[0]["next"] == [TEST_BLOCK]
    assert commands[1]["id"] == "query-writeback"
    assert commands[1]["argv"] == _query_argv()
    assert commands[2]["id"] == "command-3"
    assert commands[2]["metrics"] == _zero_metrics()


def _frontier_event() -> dict[str, object]:
    return {
        "id": "failure-frontier",
        "kind": "check",
        "argv": ["asp", "rust", "check", "changed", "--view", "seeds", "."],
        "next": [TEST_BLOCK],
        "metrics": {"elapsedMs": 5, "stdoutBytes": 180, "stderrBytes": 0},
    }


def _query_event() -> dict[str, object]:
    return {
        "eventId": "query/writeback",
        "command": {
            "method": "query",
            "query": (
                "asp rust query --from-hook direct-source-read "
                f"--selector {WRITEBACK_BLOCK} --code ."
            ),
        },
        "result": {"elapsedMs": 3, "stdoutBytes": 120, "stderrBytes": 0},
    }


def _query_argv() -> list[str]:
    return [
        "asp",
        "rust",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        WRITEBACK_BLOCK,
        "--code",
        ".",
    ]


def _summary() -> dict[str, int]:
    return {
        "commandCount": 3,
        "stdoutBytes": 300,
        "stderrBytes": 0,
        "elapsedMs": 8,
        "jsonSearches": 0,
        "compactSearches": 0,
    }


def _zero_metrics() -> dict[str, int]:
    return {"elapsedMs": 0, "stdoutBytes": 0, "stderrBytes": 0}
