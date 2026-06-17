"""Validate Claude SDK message summarization for sandtable audits."""

from __future__ import annotations

from tools.semantic_sandtable.agent_observations import summarize_agent_messages


def test_agent_summary_extracts_token_cost_and_complex_pipe_flow() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "name": "Bash",
                        "input": {
                            "command": "asp rust search prime --workspace . --view seeds",
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust search pipe 'Vec scalar' --workspace . --view seeds"
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust search reasoning owner-query "
                                "--owner src/lib.rs --query 'Vec scalar' --workspace . --view seeds"
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust query --selector src/lib.rs:1:12 "
                                "--treesitter-query '(function_item)' --code ."
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": "asp rust guide --workspace .",
                        },
                    },
                ],
            },
            {
                "type": "ResultMessage",
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 25,
                    "cache_read_input_tokens": 50,
                },
                "total_cost_usd": 0.0123,
            },
        ]
    )

    assert summary["tokenCost"]["inputTokens"] == 100
    assert summary["tokenCost"]["outputTokens"] == 25
    assert summary["tokenCost"]["cacheReadInputTokens"] == 50
    assert summary["tokenCost"]["costUsd"] == 0.0123
    assert summary["pipeFlow"]["aspCommands"] == 5
    assert summary["pipeFlow"]["searchCommands"] == 3
    assert summary["pipeFlow"]["queryCommands"] == 1
    assert summary["pipeFlow"]["guideCommands"] == 1
    assert summary["pipeFlow"]["searchPipeCommands"] == 1
    assert summary["pipeFlow"]["searchReasoningCommands"] == 1
    assert summary["pipeFlow"]["searchPrimeCommands"] == 1
    assert summary["pipeFlow"]["querySelectorCommands"] == 1
    assert summary["pipeFlow"]["treesitterQueryCommands"] == 1
    assert summary["pipeFlow"]["complexPipeFlow"]
    assert summary["finalAnswer"]["present"] is False


def test_agent_summary_extracts_final_answer_after_last_tool_use() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "name": "Bash",
                        "input": {"command": "asp rust search prime --workspace . --view seeds"},
                    }
                ],
            },
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "text": (
                            "Vec<scalar> fields are collection fields: the Vec "
                            "container owns repeated scalar elements, so the field is "
                            "not modeled as one ordinary scalar value."
                        )
                    }
                ],
            },
        ]
    )

    assert summary["finalAnswer"]["present"] is True
    assert summary["finalAnswer"]["afterLastToolUse"] is True
    assert summary["finalAnswer"]["textBytes"] > 80
    assert "Vec<scalar>" in summary["finalAnswer"]["textPreview"]


def test_agent_summary_extracts_result_message_as_final_answer() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "name": "Bash",
                        "input": {"command": "asp rust search prime --workspace . --view seeds"},
                    }
                ],
            },
            {
                "type": "ResultMessage",
                "result": (
                    "AsyncRead and AsyncWrite behavior is located through the "
                    "io owner frontier; poll_read and poll_write should be "
                    "verified from the selected ASP locator before editing."
                ),
            },
        ]
    )

    assert summary["finalAnswer"]["present"] is True
    assert summary["finalAnswer"]["afterLastToolUse"] is True
    assert "AsyncRead" in summary["finalAnswer"]["textPreview"]


def test_agent_summary_extracts_read_loop_risk_from_direct_code_reads() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust query --from-hook direct-source-read "
                                "--selector src/lib.rs:1:10 --code ."
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust query --from-hook direct-source-read "
                                "--selector src/lib.rs:11:20 --code ."
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust query --from-hook direct-source-read "
                                "--selector src/lib.rs:11:20 --code ."
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust query --from-hook direct-source-read "
                                "--selector src/lib.rs:30:35 --code ."
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust query --from-hook direct-source-read "
                                "--selector tests/test_lib.rs:1:4 --code ."
                            ),
                        },
                    },
                ],
            },
        ]
    )

    assert summary["pipeFlow"]["directReadCommands"] == 5
    assert summary["pipeFlow"]["directReadBoundedCommands"] == 5
    assert summary["pipeFlow"]["directReadBroadCommands"] == 0
    assert summary["pipeFlow"]["directReadUnboundedCommands"] == 0
    assert summary["pipeFlow"]["directReadRiskCommands"] == 0
    assert summary["pipeFlow"]["readLoopDirectCodeCommands"] == 5
    assert summary["pipeFlow"]["readLoopDuplicateSelectors"] == 1
    assert summary["pipeFlow"]["readLoopAdjacentRangeWindows"] == 1
    assert summary["pipeFlow"]["readLoopSameOwnerScans"] == 2


def test_agent_summary_classifies_broad_direct_reads_as_risk() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust query --from-hook direct-source-read "
                                "--selector src/lib.rs:1:120 --code ."
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust query --from-hook direct-source-read --code ."
                            ),
                        },
                    },
                ],
            },
        ]
    )

    assert summary["pipeFlow"]["directReadCommands"] == 2
    assert summary["pipeFlow"]["directReadBoundedCommands"] == 0
    assert summary["pipeFlow"]["directReadBroadCommands"] == 1
    assert summary["pipeFlow"]["directReadUnboundedCommands"] == 1
    assert summary["pipeFlow"]["directReadRiskCommands"] == 2
