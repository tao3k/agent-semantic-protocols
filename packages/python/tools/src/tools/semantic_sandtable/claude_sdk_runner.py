"""Run Claude Code SDK prompts for live sandtable agent scenarios."""

from __future__ import annotations

import argparse
import asyncio
import dataclasses
import os
import sys
from typing import Any

try:
    from claude_code_sdk import ClaudeCodeOptions, query
except ModuleNotFoundError:  # pragma: no cover - exercised through import-only tests.
    ClaudeCodeOptions = None  # type: ignore[assignment]
    query = None  # type: ignore[assignment]

from .agent_observations import summarize_agent_messages
from .claude_sdk_permissions import asp_bash_permission_for_budget
from .output import emit_json, emit_json_line, emit_text


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    return asyncio.run(_run(args))


async def _run(args: argparse.Namespace) -> int:
    if ClaudeCodeOptions is None or query is None:
        raise RuntimeError(
            "claude_code_sdk is required for live Claude SDK sandtable runs"
        )
    if args.require_asp_bash_commands and args.allowed_tools:
        raise ValueError(
            "--allowed-tool cannot be combined with --require-asp-bash-commands"
        )
    process_cwd = os.getcwd()
    claude_cwd = args.claude_cwd or process_cwd
    add_dirs = list(args.add_dirs or [])
    if args.add_cwd_dir and process_cwd not in add_dirs:
        add_dirs.append(process_cwd)
    prompt_text = _prompt_with_target_context(
        args.prompt,
        process_cwd,
        claude_cwd,
        add_dirs,
    )
    asp_permission = (
        asp_bash_permission_for_budget(args.max_asp_bash_commands)
        if args.require_asp_bash_commands
        else None
    )
    options = ClaudeCodeOptions(
        cwd=claude_cwd,
        settings=_settings_path(args.settings, claude_cwd),
        add_dirs=add_dirs,
        model=args.model,
        permission_mode=_permission_mode(args),
        allowed_tools=args.allowed_tools or [],
        disallowed_tools=args.disallowed_tools or [],
        include_partial_messages=args.include_partial_messages,
        extra_args=_extra_args(args),
        can_use_tool=asp_permission,
        max_turns=args.max_turns,
    )
    messages: list[dict[str, Any]] = []
    prompt = (
        _streaming_prompt(prompt_text)
        if args.require_asp_bash_commands
        else prompt_text
    )
    async for message in query(prompt=prompt, options=options):
        payload = _message_payload(message)
        messages.append(payload)
        if args.output_format == "stream-json":
            emit_json_line(payload, flush=True)
        if asp_permission is not None and asp_permission.budget_exhausted:
            budget_payload = _budget_exhausted_payload(
                asp_permission.asp_command_count
            )
            messages.append(budget_payload)
            if args.output_format == "stream-json":
                emit_json_line(budget_payload, flush=True)
            break

    summary = summarize_agent_messages(messages)
    messages.append(summary)
    _emit_final_output(args.output_format, messages, summary)
    return 0


def _emit_final_output(
    output_format: str,
    messages: list[dict[str, Any]],
    summary: dict[str, Any],
) -> None:
    if output_format in {"stream-json", "summary-json"}:
        emit_json_line(summary, flush=True)
    elif output_format == "json":
        emit_json(messages)
    elif output_format == "text":
        emit_text(_text_output(messages))


def _budget_exhausted_payload(asp_command_count: int) -> dict[str, Any]:
    return {
        "type": "SandtableAgentBudgetStop",
        "aspCommandCount": asp_command_count,
        "reason": "max-asp-bash-commands",
    }


def _extra_args(args: argparse.Namespace) -> dict[str, str | None]:
    extra_args: dict[str, str | None] = {}
    if args.include_hook_events:
        extra_args["include-hook-events"] = None
    if args.verbose:
        extra_args["verbose"] = None
    return extra_args


def _permission_mode(args: argparse.Namespace) -> str | None:
    if args.permission_mode:
        return args.permission_mode
    if args.require_asp_bash_commands:
        return "bypassPermissions"
    return None


def _settings_path(settings: str | None, claude_cwd: str) -> str | None:
    if not settings:
        return None
    if os.path.isabs(settings):
        return settings
    return os.path.join(claude_cwd, settings)


def _prompt_with_target_context(
    prompt: str,
    process_cwd: str,
    claude_cwd: str,
    add_dirs: list[str],
) -> str:
    if process_cwd == claude_cwd or process_cwd not in add_dirs:
        return prompt
    return (
        f"Sandtable target directory: {process_cwd}\n"
        "Use that directory for language-provider commands, for example "
        f"`cd {process_cwd} && asp <language> guide .`.\n\n"
        f"{prompt}"
    )


async def _streaming_prompt(prompt: str):
    yield {
        "type": "user",
        "message": {"role": "user", "content": prompt},
        "parent_tool_use_id": None,
        "session_id": "semantic-sandtable-agent-sdk",
    }


def _message_payload(message: Any) -> dict[str, Any]:
    if dataclasses.is_dataclass(message):
        payload = dataclasses.asdict(message)
    else:
        payload = {"value": str(message)}
    payload["type"] = type(message).__name__
    return payload


def _text_output(messages: list[dict[str, Any]]) -> str:
    chunks: list[str] = []
    for message in messages:
        if message.get("type") != "AssistantMessage":
            continue
        content = message.get("content")
        if not isinstance(content, list):
            continue
        for block in content:
            if isinstance(block, dict) and isinstance(block.get("text"), str):
                chunks.append(block["text"])
    return "\n".join(chunks)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prompt", required=True)
    parser.add_argument(
        "--output-format",
        required=True,
        choices=["text", "json", "stream-json", "summary-json"],
    )
    parser.add_argument("--include-partial-messages", action="store_true")
    parser.add_argument("--include-hook-events", action="store_true")
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument("--model")
    parser.add_argument(
        "--permission-mode",
        choices=["default", "acceptEdits", "plan", "bypassPermissions"],
    )
    parser.add_argument("--allowed-tool", action="append", dest="allowed_tools")
    parser.add_argument("--disallowed-tool", action="append", dest="disallowed_tools")
    parser.add_argument("--require-asp-bash-commands", action="store_true")
    parser.add_argument("--max-asp-bash-commands", type=int)
    parser.add_argument("--settings")
    parser.add_argument("--claude-cwd")
    parser.add_argument("--add-dir", action="append", dest="add_dirs")
    parser.add_argument("--add-cwd-dir", action="store_true")
    parser.add_argument("--max-turns", type=int)
    return parser


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
