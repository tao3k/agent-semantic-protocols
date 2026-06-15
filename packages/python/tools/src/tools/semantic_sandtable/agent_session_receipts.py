"""Build compact receipts from agent-session event artifacts."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .agent_observation_asp import command_contains_asp, normalize_command
from .agent_observations import summarize_agent_messages
from .agent_session_model import AgentSessionConfig
from .direct_read_shape import direct_source_read_shape
from .trace_receipt_events import TraceCommandFilter, TraceCommandParser
from .utils import dict_value, list_value, optional_int, require_str


def build_agent_session_receipt(
    session_root: Path,
    *,
    config: AgentSessionConfig | None = None,
) -> dict[str, Any]:
    manifest = _load_json_object(session_root / "manifest.json")
    if config is None:
        config = _config_from_manifest(manifest)
    events = _load_jsonl(session_root / "events.jsonl")
    messages = _load_jsonl(session_root / "messages.jsonl")
    observation = summarize_agent_messages(messages)
    commands = _receipt_commands_from_events(events)
    answer = _receipt_answer(events, observation)
    receipt = _base_receipt(session_root, config, events, commands, answer, observation)
    if config.model:
        receipt["model"] = config.model
    return receipt


def write_agent_session_receipt(
    session_root: Path,
    output_path: Path,
    *,
    config: AgentSessionConfig | None = None,
) -> dict[str, Any]:
    receipt = build_agent_session_receipt(session_root, config=config)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    _write_json(output_path, receipt)
    return receipt


def sandtable_receipt_from_agent_session(
    receipt: dict[str, Any],
) -> dict[str, Any]:
    summary = dict_value(receipt.get("summary"))
    return {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
        "schemaVersion": "1",
        "scenarioId": require_str(receipt, "scenarioId", "recorded.agent-session"),
        "language": require_str(receipt, "language", "unknown"),
        "project": dict_value(receipt.get("project")) or {"name": "unknown"},
        "intent": require_str(receipt, "intent", "Recorded agent session."),
        "editBoundary": require_str(receipt, "editBoundary", "before-edit"),
        "agentSessionId": require_str(receipt, "sessionId", "unknown"),
        "agentSessionReceiptPath": str(receipt.get("artifactRoot", "")),
        "commands": [
            _sandtable_command(command)
            for command in list_value(receipt.get("commands"))
        ],
        "summary": _sandtable_summary(summary),
        "answer": dict_value(receipt.get("answer")),
        "qualityFindings": list_value(receipt.get("qualityFindings")),
    }


def _base_receipt(
    session_root: Path,
    config: AgentSessionConfig,
    events: list[dict[str, Any]],
    commands: list[dict[str, Any]],
    answer: dict[str, Any],
    observation: dict[str, Any],
) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-receipt",
        "schemaVersion": "1",
        "sessionId": config.session_id,
        "scenarioId": config.scenario_id,
        "language": config.language,
        "project": _project(config),
        "intent": config.intent,
        "agent": config.agent,
        "startedAtUtc": _first_event_time(events),
        "finishedAtUtc": _last_event_time(events),
        "editBoundary": config.edit_boundary,
        "artifactRoot": str(session_root),
        "summary": _receipt_summary(events, observation, commands),
        "answer": answer,
        "commands": commands,
        "qualityFindings": _initial_quality_findings(answer),
        "artifactRefs": [
            {"kind": "events", "path": "events.jsonl"},
            {"kind": "messages", "path": "messages.jsonl"},
        ],
    }


def _receipt_commands_from_events(events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    commands = []
    parser = TraceCommandParser(filters=TraceCommandFilter())
    for event in events:
        if event.get("kind") != "command.result":
            continue
        fields = dict_value(event.get("fields"))
        argv = [str(item) for item in list_value(fields.get("argv"))]
        if not argv:
            argv = parser.split_command_line(str(fields.get("command", "unknown")))
        command = _receipt_command(event, fields, argv)
        commands.append(command)
    return commands


def _receipt_command(
    event: dict[str, Any],
    fields: dict[str, Any],
    argv: list[str],
) -> dict[str, Any]:
    command = {
        "id": require_str(event, "commandId", require_str(event, "eventId", "command")),
        "kind": _command_kind(argv, bool(fields.get("denied"))),
        "argv": argv or ["unknown"],
        "outputMode": "json" if "--json" in argv else "compact",
        "metrics": _command_metrics(fields),
        "eventId": require_str(event, "eventId", "unknown"),
    }
    if event.get("toolUseId"):
        command["toolUseId"] = str(event["toolUseId"])
    if fields.get("denied"):
        command["denied"] = True
    artifacts = _output_artifacts(event)
    if artifacts:
        command["outputArtifacts"] = artifacts
    return command


def _command_metrics(fields: dict[str, Any]) -> dict[str, int]:
    return {
        "elapsedMs": 0,
        "stdoutBytes": optional_int(fields.get("stdoutBytes")) or 0,
        "stderrBytes": optional_int(fields.get("stderrBytes")) or 0,
        "stdoutLines": optional_int(fields.get("stdoutLines")) or 0,
        "stderrLines": optional_int(fields.get("stderrLines")) or 0,
    }


def _receipt_summary(
    events: list[dict[str, Any]],
    observation: dict[str, Any],
    commands: list[dict[str, Any]],
) -> dict[str, Any]:
    pipe_flow = dict_value(observation.get("pipeFlow"))
    summary = {
        "turns": len(events),
        "assistantVisibleMessages": _event_count(events, "assistant.visible-message"),
        "toolRequests": _event_count(events, "tool.request"),
        "toolResults": _event_count(events, "tool.result"),
        "commandCount": len(commands),
        "aspCommands": optional_int(pipe_flow.get("aspCommands")) or _asp_command_count(commands),
        "searchCommands": optional_int(pipe_flow.get("searchCommands"))
        or _surface_count(commands, "search"),
        "queryCommands": optional_int(pipe_flow.get("queryCommands"))
        or _surface_count(commands, "query"),
        "checkCommands": optional_int(pipe_flow.get("checkCommands"))
        or _surface_count(commands, "check"),
        "guideCommands": optional_int(pipe_flow.get("guideCommands"))
        or _surface_count(commands, "guide"),
        "deniedCommands": optional_int(pipe_flow.get("deniedAspCommands"))
        or _denied_count(commands),
        "repeatedCommands": optional_int(pipe_flow.get("repeatedCommands"))
        or _repeated_count(commands),
        "directReadRiskCommands": optional_int(pipe_flow.get("directReadRiskCommands"))
        or _direct_read_risk_count(commands),
        "stdoutBytes": sum(_metric(command, "stdoutBytes") for command in commands),
        "stderrBytes": sum(_metric(command, "stderrBytes") for command in commands),
        "elapsedMs": sum(_metric(command, "elapsedMs") for command in commands),
    }
    _attach_token_cost(summary, observation)
    _attach_pipe_flow_counts(summary, pipe_flow)
    return summary


def _attach_pipe_flow_counts(summary: dict[str, Any], pipe_flow: dict[str, Any]) -> None:
    for key in (
        "searchPrimeCommands",
        "searchPipeCommands",
        "querySelectorCommands",
        "directReadCommands",
    ):
        value = optional_int(pipe_flow.get(key))
        if value is not None:
            summary[key] = value


def _attach_token_cost(summary: dict[str, Any], observation: dict[str, Any]) -> None:
    token_cost = _token_cost(dict_value(observation.get("tokenCost")))
    if token_cost:
        summary["tokenCost"] = token_cost


def _receipt_answer(
    events: list[dict[str, Any]],
    observation: dict[str, Any],
) -> dict[str, Any]:
    final_answer = dict_value(observation.get("finalAnswer"))
    answer_event = _last_event(events, "answer.final")
    evidence_refs = _answer_evidence_refs(events)
    present = bool(final_answer.get("present"))
    return {
        "present": present,
        "afterLastToolUse": bool(final_answer.get("afterLastToolUse")),
        "textBytes": optional_int(final_answer.get("textBytes")) or 0,
        "textLineCount": optional_int(final_answer.get("textLineCount")) or 0,
        "messageEventId": require_str(answer_event, "eventId", "answer-final"),
        "messageIndex": optional_int(final_answer.get("messageIndex")) or 0,
        "evidenceRefs": evidence_refs,
        "groundingStatus": _grounding_status(present, evidence_refs),
        "preview": str(final_answer.get("textPreview", "")),
    }


def _sandtable_command(command: Any) -> dict[str, Any]:
    if not isinstance(command, dict):
        return {"id": "command", "kind": "other", "argv": ["unknown"], "metrics": _zero_metrics()}
    item = dict(command)
    supported = {"search", "hook-deny", "subagent", "external-ingest", "check", "other"}
    if item.get("kind") not in supported:
        item["kind"] = "other"
    return item


def _sandtable_summary(summary: dict[str, Any]) -> dict[str, int]:
    return {
        "commandCount": optional_int(summary.get("commandCount")) or 0,
        "stdoutBytes": optional_int(summary.get("stdoutBytes")) or 0,
        "stderrBytes": optional_int(summary.get("stderrBytes")) or 0,
        "elapsedMs": optional_int(summary.get("elapsedMs")) or 0,
        "aspCommands": optional_int(summary.get("aspCommands")) or 0,
        "searchCommands": optional_int(summary.get("searchCommands")) or 0,
        "queryCommands": optional_int(summary.get("queryCommands")) or 0,
        "directReadCommands": optional_int(summary.get("directReadCommands")) or 0,
        "directReadBoundedCommands": 0,
        "directReadBroadCommands": 0,
        "directReadUnboundedCommands": 0,
        "directReadRiskCommands": optional_int(summary.get("directReadRiskCommands"))
        or 0,
        "repeatedCommands": optional_int(summary.get("repeatedCommands")) or 0,
        "repeatedSearches": 0,
        "jsonSearches": 0,
        "compactSearches": optional_int(summary.get("searchCommands")) or 0,
    }


def _initial_quality_findings(answer: dict[str, Any]) -> list[dict[str, Any]]:
    if answer.get("present"):
        return []
    return [
        {
            "id": "answer.missing",
            "kind": "answer-grounding",
            "severity": "error",
            "message": "No final answer was emitted after the last visible tool use.",
            "recommendedAction": "Ensure the live agent run reaches an answer.final event.",
        }
    ]


def _answer_evidence_refs(events: list[dict[str, Any]]) -> list[str]:
    refs = []
    for event in events:
        if event.get("kind") == "command.result":
            refs.append(require_str(event, "commandId", require_str(event, "eventId", "")))
    return refs[:12]


def _grounding_status(present: bool, evidence_refs: list[str]) -> str:
    if not present:
        return "unknown"
    return "grounded" if evidence_refs else "weak"


def _command_kind(argv: list[str], denied: bool) -> str:
    if denied:
        return "hook-deny"
    for kind in ("search", "query", "check", "guide"):
        if kind in argv:
            return kind
    return "other"


def _output_artifacts(event: dict[str, Any]) -> dict[str, str]:
    artifacts = {}
    for ref in list_value(event.get("artifactRefs")):
        if not isinstance(ref, dict):
            continue
        if ref.get("kind") == "stdout":
            artifacts["stdoutPath"] = str(ref.get("path", ""))
        elif ref.get("kind") == "stderr":
            artifacts["stderrPath"] = str(ref.get("path", ""))
    return {key: value for key, value in artifacts.items() if value}


def _token_cost(value: dict[str, Any]) -> dict[str, Any]:
    total = optional_int(value.get("totalTokens"))
    if total is None:
        return {}
    token_cost = {"unit": "token-count", "totalTokens": total, "basis": "claude-sdk-stream"}
    for source in ("inputTokens", "outputTokens", "cacheReadInputTokens", "costUsd"):
        if source in value:
            token_cost[source] = value[source]
    return token_cost


def _metric(command: dict[str, Any], key: str) -> int:
    return optional_int(dict_value(command.get("metrics")).get(key)) or 0


def _event_count(events: list[dict[str, Any]], kind: str) -> int:
    return sum(1 for event in events if event.get("kind") == kind)


def _asp_command_count(commands: list[dict[str, Any]]) -> int:
    return sum(1 for command in commands if command_contains_asp(" ".join(command["argv"])))


def _surface_count(commands: list[dict[str, Any]], surface: str) -> int:
    return sum(1 for command in commands if surface in command.get("argv", []))


def _denied_count(commands: list[dict[str, Any]]) -> int:
    return sum(1 for command in commands if command.get("denied"))


def _repeated_count(commands: list[dict[str, Any]]) -> int:
    counts: dict[str, int] = {}
    for command in commands:
        normalized = normalize_command(" ".join(str(arg) for arg in command.get("argv", [])))
        counts[normalized] = counts.get(normalized, 0) + 1
    return sum(count - 1 for count in counts.values() if count > 1)


def _direct_read_risk_count(commands: list[dict[str, Any]]) -> int:
    count = 0
    for command in commands:
        argv = [str(arg) for arg in command.get("argv", [])]
        if "direct-source-read" in argv and direct_source_read_shape(argv) in {
            "broad",
            "unbounded",
        }:
            count += 1
    return count


def _first_event_time(events: list[dict[str, Any]]) -> str:
    if events:
        return require_str(events[0], "timestampUtc", "1970-01-01T00:00:00Z")
    return "1970-01-01T00:00:00Z"


def _last_event_time(events: list[dict[str, Any]]) -> str:
    if events:
        return require_str(events[-1], "timestampUtc", "1970-01-01T00:00:00Z")
    return "1970-01-01T00:00:00Z"


def _last_event(events: list[dict[str, Any]], kind: str) -> dict[str, Any]:
    for event in reversed(events):
        if event.get("kind") == kind:
            return event
    return {}


def _project(config: AgentSessionConfig) -> dict[str, str]:
    project = {"name": config.project_name, "source": config.project_source}
    if config.project_workdir:
        project["workdir"] = config.project_workdir
    return project


def _config_from_manifest(manifest: dict[str, Any]) -> AgentSessionConfig:
    project = dict_value(manifest.get("project"))
    return AgentSessionConfig(
        session_id=require_str(manifest, "sessionId", "agent-session"),
        scenario_id=require_str(manifest, "scenarioId", "recorded.agent-session"),
        language=require_str(manifest, "language", "unknown"),
        project_name=require_str(project, "name", "unknown"),
        project_source=require_str(project, "source", "unknown"),
        project_workdir=project.get("workdir")
        if isinstance(project.get("workdir"), str)
        else None,
        intent=require_str(manifest, "intent", "Recorded agent session."),
        agent=require_str(manifest, "agent", "unknown"),
        model=manifest.get("model") if isinstance(manifest.get("model"), str) else None,
        edit_boundary=require_str(manifest, "editBoundary", "before-edit"),
    )


def _zero_metrics() -> dict[str, int]:
    return {"elapsedMs": 0, "stdoutBytes": 0, "stderrBytes": 0}


def _write_json(path: Path, item: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(item, handle, indent=2, sort_keys=True)
        handle.write("\n")


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    items = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            try:
                value = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(value, dict):
                items.append(value)
    return items


def _load_json_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}
