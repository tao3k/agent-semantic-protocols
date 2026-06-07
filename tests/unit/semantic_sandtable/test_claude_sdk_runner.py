"""Validate the Claude Code SDK sandtable runner helpers."""

from __future__ import annotations

import asyncio
from argparse import Namespace

from tools.semantic_sandtable.agent_observations import summarize_agent_messages
from tools.semantic_sandtable.claude_sdk_runner import (
    _asp_bash_permission,
    _extra_args,
    _is_asp_command,
    _text_output,
)


def test_extra_args_projects_hook_and_verbose_flags() -> None:
    args = Namespace(include_hook_events=True, verbose=True)

    assert _extra_args(args) == {
        "include-hook-events": None,
        "verbose": None,
    }


def test_text_output_projects_assistant_text_blocks_only() -> None:
    messages = [
        {
            "type": "SystemMessage",
            "content": [{"text": "ignore"}],
        },
        {
            "type": "AssistantMessage",
            "content": [
                {"text": "first"},
                {"name": "tool-call"},
                {"text": "second"},
            ],
        },
    ]

    assert _text_output(messages) == "first\nsecond"


def test_is_asp_command_accepts_facade_and_workspace_binary() -> None:
    assert _is_asp_command("asp rust guide .")
    assert _is_asp_command("/workspace/.bin/asp rust search prime .")
    assert _is_asp_command("cd /workspace && asp rust search prime --view seeds .")
    assert _is_asp_command(
        "direnv exec /workspace asp rust query --selector src/lib.rs:1:4 --code ."
    )
    assert not _is_asp_command("grep -R Vec .")


def test_asp_bash_permission_rejects_raw_shell() -> None:
    allowed = asyncio.run(
        _asp_bash_permission("Bash", {"command": "asp rust guide ."}, None)
    )
    denied = asyncio.run(
        _asp_bash_permission("Bash", {"command": "grep -R Vec ."}, None)
    )
    non_bash = asyncio.run(_asp_bash_permission("Grep", {"pattern": "Vec"}, None))

    assert allowed.behavior == "allow"
    assert denied.behavior == "deny"
    assert non_bash.behavior == "deny"


def test_agent_summary_extracts_token_cost_and_complex_pipe_flow() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "name": "Bash",
                        "input": {
                            "command": "asp rust search prime --view seeds .",
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust search pipe 'Vec scalar' --view seeds ."
                            ),
                        },
                    },
                    {
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp rust search reasoning owner-query "
                                "--owner src/lib.rs --query 'Vec scalar' --view seeds ."
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
                            "command": "asp rust guide .",
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
    assert summary["pipeFlow"]["readLoopDirectCodeCommands"] == 5
    assert summary["pipeFlow"]["readLoopDuplicateSelectors"] == 1
    assert summary["pipeFlow"]["readLoopAdjacentRangeWindows"] == 1
    assert summary["pipeFlow"]["readLoopSameOwnerScans"] == 2
