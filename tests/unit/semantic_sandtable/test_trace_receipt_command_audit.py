"""Audit semantic command counts in trace receipts."""

from __future__ import annotations

from tools.semantic_sandtable.trace_receipts import (
    TraceReceiptConfig,
    build_receipt_from_trace_path,
)


def test_trace_receipt_summary_counts_semantic_search_query_commands(tmp_path) -> None:
    trace_path = tmp_path / "commands.jsonl"
    trace_path.write_text(
        "\n".join(
            [
                "$ asp rust search prime --workspace . --view seeds",
                "$ asp rust query --term Vec .",
                "$ asp rust query --from-hook direct-source-read --selector src/lib.rs:1-2 .",
                "$ asp rust search prime --workspace . --view seeds",
                "$ python helper.py",
            ]
        )
        + "\n"
    )

    receipt = build_receipt_from_trace_path(
        trace_path,
        config=TraceReceiptConfig(
            scenario_id="rust.tokio-claude-deep-question-flow",
            language="rust",
            project_name="tokio",
            intent="audit semantic command count",
        ),
    )

    assert receipt["summary"]["commandCount"] == 5
    assert receipt["summary"]["aspCommands"] == 4
    assert receipt["summary"]["searchCommands"] == 2
    assert receipt["summary"]["queryCommands"] == 2
    assert receipt["summary"]["directReadCommands"] == 1
    assert receipt["summary"]["repeatedCommands"] == 1
    assert receipt["summary"]["repeatedSearches"] == 1
    assert receipt["summary"]["compactSearches"] == 2
