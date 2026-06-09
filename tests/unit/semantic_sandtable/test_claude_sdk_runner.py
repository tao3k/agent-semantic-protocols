"""Validate the Claude Code SDK sandtable runner helpers."""

from __future__ import annotations

import asyncio
import json
from argparse import Namespace

from tools.semantic_sandtable.claude_sdk_permissions import (
    asp_bash_permission,
    is_asp_command,
)
from tools.semantic_sandtable.claude_sdk_runner import (
    _budget_exhausted_payload,
    _claude_env,
    _emit_final_output,
    _extra_args,
    _permission_mode,
    _text_output,
)


def test_extra_args_projects_hook_and_verbose_flags() -> None:
    args = Namespace(include_hook_events=True, verbose=True)

    assert _extra_args(args) == {
        "include-hook-events": None,
        "verbose": None,
    }


def test_require_asp_bash_defaults_to_bypass_permission_mode() -> None:
    assert (
        _permission_mode(
            Namespace(permission_mode=None, require_asp_bash_commands=True)
        )
        == "bypassPermissions"
    )
    assert (
        _permission_mode(
            Namespace(permission_mode="default", require_asp_bash_commands=True)
        )
        == "default"
    )
    assert (
        _permission_mode(
            Namespace(permission_mode=None, require_asp_bash_commands=False)
        )
        is None
    )


def test_max_asp_bash_commands_projects_hook_budget_env() -> None:
    assert _claude_env(Namespace(max_asp_bash_commands=3)) == {
        "ASP_HOOK_MAX_ASP_COMMANDS": "3"
    }
    assert _claude_env(Namespace(max_asp_bash_commands=None)) == {}


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


def test_summary_json_outputs_only_summary_record(capsys) -> None:
    messages = [
        {
            "type": "AssistantMessage",
            "content": [{"text": "large final answer body"}],
        }
    ]
    summary = {"type": "SandtableAgentSdkSummary", "finalAnswer": {"present": True}}

    _emit_final_output("summary-json", messages, summary)

    output = capsys.readouterr().out.strip().splitlines()
    assert len(output) == 1
    assert json.loads(output[0]) == summary


def test_budget_exhausted_payload_records_hard_stop_reason() -> None:
    assert _budget_exhausted_payload(3) == {
        "type": "SandtableAgentBudgetStop",
        "aspCommandCount": 3,
        "reason": "max-asp-bash-commands",
    }


def test_is_asp_command_accepts_facade_and_workspace_binary() -> None:
    assert is_asp_command("asp rust guide .")
    assert is_asp_command("/workspace/.bin/asp rust search prime .")
    assert is_asp_command("cd /workspace && asp rust search prime --view seeds .")
    assert is_asp_command(
        "direnv exec /workspace asp rust query --selector src/lib.rs:1:4 --code ."
    )
    assert not is_asp_command("grep -R Vec .")


def test_asp_bash_permission_rejects_raw_shell() -> None:
    allowed = asyncio.run(
        asp_bash_permission("Bash", {"command": "asp rust guide ."}, None)
    )
    denied = asyncio.run(
        asp_bash_permission("Bash", {"command": "grep -R Vec ."}, None)
    )
    non_bash = asyncio.run(asp_bash_permission("Grep", {"pattern": "Vec"}, None))

    assert allowed.behavior == "allow"
    assert denied.behavior == "deny"
    assert non_bash.behavior == "deny"
