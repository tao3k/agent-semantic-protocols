"""Project Claude SDK messages into agent-session event artifacts."""

from __future__ import annotations

import hashlib
import json
import re
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from .agent_observation_json import load_stdout_messages
from .agent_observations import summarize_agent_messages
from .agent_session_model import AgentSessionConfig
from .trace_receipt_events import TraceCommandFilter, TraceCommandParser
from .utils import dict_value, list_value, optional_int, require_str


def load_agent_messages(path: Path) -> list[dict[str, Any]]:
    return load_stdout_messages(path.read_text(encoding="utf-8"))


def write_agent_session_from_messages(
    messages: list[dict[str, Any]],
    session_root: Path,
    *,
    config: AgentSessionConfig,
) -> dict[str, Any]:
    session_root.mkdir(parents=True, exist_ok=True)
    _write_jsonl(session_root / "messages.jsonl", messages)
    events = agent_session_events_from_messages(messages, config=config)
    _write_jsonl(session_root / "events.jsonl", events)
    _write_command_artifacts(session_root, events, messages)
    manifest = _session_manifest(config, session_root, events)
    _write_json(session_root / "manifest.json", manifest)
    return manifest


def agent_session_events_from_messages(
    messages: list[dict[str, Any]],
    *,
    config: AgentSessionConfig,
) -> list[dict[str, Any]]:
    events = _EventBuilder(config)
    events.add(
        "session.start",
        fields={
            "scenarioId": config.scenario_id,
            "language": config.language,
            "agent": config.agent,
        },
    )
    events.add("user.intent", preview=config.intent, fields={"intent": config.intent})

    command_by_tool_id: dict[str, str] = {}
    for message_index, message in enumerate(messages):
        events.add_message_events(message, message_index, command_by_tool_id)

    summary = summarize_agent_messages(messages)
    answer = dict_value(summary.get("finalAnswer"))
    if answer:
        events.add(
            "answer.final",
            preview=str(answer.get("textPreview", "")),
            fields={
                "present": bool(answer.get("present")),
                "afterLastToolUse": bool(answer.get("afterLastToolUse")),
                "textBytes": optional_int(answer.get("textBytes")) or 0,
                "textLineCount": optional_int(answer.get("textLineCount")) or 0,
            },
        )
    events.add("session.stop", fields={"status": "complete" if answer else "partial"})
    return events.items


class _EventBuilder:
    def __init__(self, config: AgentSessionConfig) -> None:
        self.config = config
        self.items: list[dict[str, Any]] = []

    def add(
        self,
        kind: str,
        *,
        source: str = "semantic-sandtable",
        preview: str = "",
        fields: dict[str, Any] | None = None,
        message_index: int | None = None,
        tool_use_id: str | None = None,
        command_id: str | None = None,
        artifact_refs: list[dict[str, Any]] | None = None,
    ) -> dict[str, Any]:
        event = {
            "schemaId": "agent.semantic-protocols.semantic-agent-session-event",
            "schemaVersion": "1",
            "eventId": _event_id(self.config.session_id, len(self.items), kind),
            "sessionId": self.config.session_id,
            "ordinal": len(self.items),
            "timestampUtc": _utc_now(),
            "kind": kind,
            "source": source,
        }
        _attach_optional_event_fields(
            event,
            preview=preview,
            fields=fields,
            message_index=message_index,
            tool_use_id=tool_use_id,
            command_id=command_id,
            artifact_refs=artifact_refs,
        )
        self.items.append(event)
        return event

    def add_message_events(
        self,
        message: dict[str, Any],
        message_index: int,
        command_by_tool_id: dict[str, str],
    ) -> None:
        for block in _content_blocks(message):
            self._add_text_event(block, message_index)
            self._add_tool_request(block, message_index, command_by_tool_id)
            self._add_tool_result(block, message_index, command_by_tool_id)

    def _add_text_event(self, block: dict[str, Any], message_index: int) -> None:
        text = block.get("text")
        if isinstance(text, str) and text.strip():
            self.add(
                "assistant.visible-message",
                source="claude-sdk",
                preview=text.strip(),
                message_index=message_index,
            )

    def _add_tool_request(
        self,
        block: dict[str, Any],
        message_index: int,
        command_by_tool_id: dict[str, str],
    ) -> None:
        command = _tool_command(block)
        tool_id = block.get("id")
        if not command or not isinstance(tool_id, str):
            return
        command_by_tool_id[tool_id] = command
        command_id = _safe_id(f"command-{tool_id}")
        self.add(
            "tool.request",
            source="claude-sdk",
            preview=command,
            message_index=message_index,
            tool_use_id=tool_id,
            command_id=command_id,
            fields={"tool": str(block.get("name", "")), "command": command},
        )
        self.add(
            "command.start",
            source="claude-sdk",
            preview=command,
            message_index=message_index,
            tool_use_id=tool_id,
            command_id=command_id,
            fields={"command": command, "argv": _split_command(command)},
        )

    def _add_tool_result(
        self,
        block: dict[str, Any],
        message_index: int,
        command_by_tool_id: dict[str, str],
    ) -> None:
        tool_use_id = block.get("tool_use_id")
        if not isinstance(tool_use_id, str):
            return
        output = _tool_result_text(block.get("content"))
        command = command_by_tool_id.get(tool_use_id, "")
        command_id = _safe_id(f"command-{tool_use_id}")
        self.add(
            "tool.result",
            source="claude-sdk",
            preview=_safe_preview(output),
            message_index=message_index,
            tool_use_id=tool_use_id,
            command_id=command_id if command else None,
            fields={"isError": bool(block.get("is_error"))},
        )
        if command:
            self.add(
                "command.result",
                source="claude-sdk",
                preview=command,
                message_index=message_index,
                tool_use_id=tool_use_id,
                command_id=command_id,
                artifact_refs=[_output_ref(command_id, "stdout", output)],
                fields=_command_result_fields(block, command, output),
            )


def _attach_optional_event_fields(
    event: dict[str, Any],
    *,
    preview: str,
    fields: dict[str, Any] | None,
    message_index: int | None,
    tool_use_id: str | None,
    command_id: str | None,
    artifact_refs: list[dict[str, Any]] | None,
) -> None:
    if preview:
        event["preview"] = preview[:500]
    if fields:
        event["fields"] = _flat_fields(fields)
    if message_index is not None:
        event["messageIndex"] = message_index
    if tool_use_id:
        event["toolUseId"] = tool_use_id
    if command_id:
        event["commandId"] = command_id
    if artifact_refs:
        event["artifactRefs"] = artifact_refs


def _command_result_fields(
    block: dict[str, Any],
    command: str,
    output: str,
) -> dict[str, Any]:
    return {
        "command": command,
        "argv": _split_command(command),
        "exitCode": 1 if block.get("is_error") else 0,
        "stdoutBytes": len(output.encode("utf-8")),
        "stdoutLines": len(output.splitlines()),
        "stderrBytes": 0,
        "stderrLines": 0,
        "denied": _is_denied_output(output),
    }


def _session_manifest(
    config: AgentSessionConfig,
    session_root: Path,
    events: list[dict[str, Any]],
) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-manifest",
        "schemaVersion": "1",
        "sessionId": config.session_id,
        "scenarioId": config.scenario_id,
        "language": config.language,
        "project": _project(config),
        "intent": config.intent,
        "agent": config.agent,
        "model": config.model,
        "editBoundary": config.edit_boundary,
        "artifactRoot": str(session_root),
        "eventCount": len(events),
    }


def _project(config: AgentSessionConfig) -> dict[str, str]:
    project = {"name": config.project_name, "source": config.project_source}
    if config.project_workdir:
        project["workdir"] = config.project_workdir
    return project


def _content_blocks(message: dict[str, Any]) -> list[dict[str, Any]]:
    blocks = message.get("content")
    if isinstance(blocks, list):
        return [block for block in blocks if isinstance(block, dict)]
    return []


def _tool_command(block: dict[str, Any]) -> str:
    tool_input = block.get("input")
    if not isinstance(tool_input, dict):
        return ""
    command = tool_input.get("command")
    return command if isinstance(command, str) and command else ""


def _tool_result_text(value: Any) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        text = value.get("text")
        if isinstance(text, str):
            return text
        return "".join(_tool_result_text(item) for item in value.values())
    if isinstance(value, list):
        return "".join(_tool_result_text(item) for item in value)
    return ""


def _split_command(command: str) -> list[str]:
    return TraceCommandParser(filters=TraceCommandFilter()).split_command_line(command)


def _output_ref(command_id: str, stream: str, text: str) -> dict[str, Any]:
    encoded = text.encode("utf-8")
    return {
        "kind": stream,
        "path": f"outputs/{command_id}.{stream}",
        "sha256": hashlib.sha256(encoded).hexdigest(),
        "bytes": len(encoded),
        "lines": len(text.splitlines()),
    }


def _write_command_artifacts(
    session_root: Path,
    events: list[dict[str, Any]],
    messages: list[dict[str, Any]],
) -> None:
    outputs = _tool_result_outputs(messages)
    output_events = [event for event in events if event.get("kind") == "command.result"]
    for event in output_events:
        command_id = require_str(event, "commandId", "command")
        _write_json(session_root / "commands" / f"{command_id}.json", event)
        _write_output_artifacts(session_root, event, outputs)


def _write_output_artifacts(
    session_root: Path,
    event: dict[str, Any],
    outputs: dict[str, str],
) -> None:
    output = outputs.get(require_str(event, "toolUseId", ""), "")
    for ref in list_value(event.get("artifactRefs")):
        if isinstance(ref, dict) and ref.get("kind") == "stdout":
            output_path = session_root / str(ref.get("path", ""))
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(output, encoding="utf-8")


def _tool_result_outputs(messages: list[dict[str, Any]]) -> dict[str, str]:
    outputs = {}
    for message in messages:
        for block in _content_blocks(message):
            tool_use_id = block.get("tool_use_id")
            if isinstance(tool_use_id, str):
                outputs[tool_use_id] = _tool_result_text(block.get("content"))
    return outputs


def _safe_preview(text: str) -> str:
    return " ".join(text.split())[:500]


def _is_denied_output(output: str) -> bool:
    return (
        "ASP hook denied" in output
        or "Command blocked by PreToolUse hook" in output
        or "hookFeedback=" in output
        or '"hookFeedback"' in output
    )


def _flat_fields(fields: dict[str, Any]) -> dict[str, Any]:
    flat: dict[str, Any] = {}
    for key, value in fields.items():
        if isinstance(value, (str, int, float, bool)):
            flat[key] = value
        elif isinstance(value, list):
            flat[key] = [
                item for item in value if isinstance(item, (str, int, float, bool))
            ]
    return flat


def _safe_id(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.:-]+", "-", value).strip("-") or "event"


def _event_id(session_id: str, ordinal: int, kind: str) -> str:
    return f"{_safe_id(session_id)}-{ordinal:04d}-{kind.replace('.', '-')}"


def _utc_now() -> str:
    return datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def _write_jsonl(path: Path, items: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for item in items:
            handle.write(json.dumps(item, sort_keys=True, separators=(",", ":")))
            handle.write("\n")


def _write_json(path: Path, item: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(item, handle, indent=2, sort_keys=True)
        handle.write("\n")
