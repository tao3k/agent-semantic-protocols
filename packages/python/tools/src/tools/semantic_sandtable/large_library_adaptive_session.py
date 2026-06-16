"""Agent-session fixtures for adaptive large-library simulation."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .agent_session import AgentSessionConfig
from .utils import dict_value, require_str


def _messages_for_results(
    run: dict[str, Any],
    results: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    messages: list[dict[str, Any]] = []
    for index, result in enumerate(results, start=1):
        tool_id = f"call_{index}"
        messages.append(_assistant_command_message(tool_id, str(result["command"])))
        messages.append(_tool_result_message(tool_id, result))
    messages.append(
        {
            "type": "ResultMessage",
            "result": _final_answer(run, results),
            "usage": {"input_tokens": 0, "output_tokens": 0},
            "total_cost_usd": 0,
        }
    )
    return messages


def _session_config(run: dict[str, Any], workdir: Path) -> AgentSessionConfig:
    project = dict_value(run.get("project"))
    return AgentSessionConfig(
        session_id=require_str(run, "runId", "unknown"),
        scenario_id=require_str(run, "scenarioId", "unknown"),
        language=require_str(run, "language", "unknown"),
        project_name=require_str(project, "name", "unknown"),
        project_source=require_str(project, "source", "registry"),
        project_workdir=str(workdir),
        intent=require_str(run, "prompt", require_str(run, "questionId", "")),
        agent="fixture",
        model="simulated-asp",
    )


def _assistant_command_message(tool_id: str, command: str) -> dict[str, Any]:
    return {
        "type": "AssistantMessage",
        "content": [{"id": tool_id, "name": "Bash", "input": {"command": command}}],
    }


def _tool_result_message(tool_id: str, result: dict[str, Any]) -> dict[str, Any]:
    output = str(result.get("stdout", ""))
    stderr = str(result.get("stderr", ""))
    if stderr:
        output = f"{output}\n[stderr]\n{stderr}"
    return {
        "type": "UserMessage",
        "content": [
            {
                "tool_use_id": tool_id,
                "content": output,
                "is_error": int(result.get("exitCode", 0)) != 0,
            }
        ],
    }


def _final_answer(run: dict[str, Any], results: list[dict[str, Any]]) -> str:
    status = "grounded" if all(result["exitCode"] == 0 for result in results) else "partial"
    return (
        f"Simulated deep search answer is {status}. Evidence came from "
        f"{len(results)} ASP commands for question {run.get('questionId')}."
    )
