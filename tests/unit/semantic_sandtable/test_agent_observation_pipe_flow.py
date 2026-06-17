"""Validate ASP pipe-flow observations from agent SDK messages."""

from __future__ import annotations

from tools.semantic_sandtable.agent_observation_asp import command_contains_asp
from tools.semantic_sandtable.agent_observations import summarize_agent_messages


def test_asp_probe_is_not_counted_as_asp_command() -> None:
    assert not command_contains_asp('which asp 2>/dev/null || echo "asp not found"')
    assert command_contains_asp("cd /repo && asp typescript search prime --workspace . --view seeds")


def test_excludes_hook_denied_commands_from_executed_pipe_flow() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_prime",
                        "name": "Bash",
                        "input": {
                            "command": "asp typescript search prime --workspace . --view seeds"
                        },
                    }
                ],
            },
            {
                "type": "UserMessage",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_prime",
                        "content": "[search-prime]\n|owner path=packages/effect/src",
                    }
                ],
            },
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_repeat_prime",
                        "name": "Bash",
                        "input": {
                            "command": "asp typescript search prime --workspace . --view seeds"
                        },
                    }
                ],
            },
            {
                "type": "UserMessage",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_repeat_prime",
                        "is_error": True,
                        "content": (
                            "ASP hook denied repeated `search prime` before `search pipe`.\n"
                            '{"hookFeedback":"repeat-prime-before-pipe"}'
                        ),
                    }
                ],
            },
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_pipe",
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp typescript search pipe 'Effect concurrency Fiber' "
                                "--workspace . --view seeds"
                            )
                        },
                    }
                ],
            },
            {
                "type": "UserMessage",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_pipe",
                        "content": (
                            "[search-pipe]\n"
                            "|queryQuality=high\n"
                            "|nextCommand=query-selector"
                        ),
                    }
                ],
            },
        ]
    )

    pipe_flow = summary["pipeFlow"]
    assert pipe_flow["aspCommands"] == 2
    assert pipe_flow["searchCommands"] == 2
    assert pipe_flow["searchPrimeCommands"] == 1
    assert pipe_flow["searchPipeCommands"] == 1
    assert pipe_flow["repeatedCommands"] == 0
    assert pipe_flow["deniedAspCommands"] == 1
    assert pipe_flow["deniedHookFeedback"] == ["repeat-prime-before-pipe"]
    assert pipe_flow["deniedCommands"] == [
        "asp typescript search prime --workspace . --view seeds"
    ]


def test_counts_non_hook_bash_errors_as_executed_asp_commands() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_prime_error",
                        "name": "Bash",
                        "input": {
                            "command": "asp typescript search prime --workspace . --view seeds"
                        },
                    }
                ],
            },
            {
                "type": "UserMessage",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_prime_error",
                        "is_error": True,
                        "content": "error: provider unavailable",
                    }
                ],
            },
        ]
    )

    pipe_flow = summary["pipeFlow"]
    assert pipe_flow["aspCommands"] == 1
    assert pipe_flow["searchCommands"] == 1
    assert pipe_flow["searchPrimeCommands"] == 1
    assert "deniedAspCommands" not in pipe_flow


def test_output_preview_omits_code_emitting_query_records() -> None:
    summary = summarize_agent_messages(
        [
            {
                "type": "AssistantMessage",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_pipe",
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp typescript search pipe 'Effect concurrency' "
                                "--workspace . --view seeds"
                            )
                        },
                    },
                    {
                        "type": "tool_use",
                        "id": "toolu_code",
                        "name": "Bash",
                        "input": {
                            "command": (
                                "asp typescript query --selector src/Fiber.ts:1:3 "
                                "--workspace . --code"
                            )
                        },
                    },
                ],
            },
            {
                "type": "UserMessage",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_pipe",
                        "content": "[search-pipe]\nnextCommand=asp rg -query 'Fiber' .",
                    },
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_code",
                        "content": "export interface Fiber {}\n",
                    },
                ],
            },
        ]
    )

    records = summary["pipeFlow"]["aspCommandOutputRecords"]
    assert records[0]["outputPreview"] == (
        "[search-pipe] nextCommand=asp rg -query 'Fiber' ."
    )
    assert "outputPreview" not in records[1]
