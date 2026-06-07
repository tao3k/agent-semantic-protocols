"""Extract compact agent observations from Claude SDK sandtable output."""

from __future__ import annotations

from typing import Any

from .agent_observation_json import last_summary, load_stdout_messages
from .agent_observation_pipe import pipe_flow_from_messages
from .agent_observation_tokens import token_cost_from_messages


def summarize_agent_stdout(stdout: str) -> dict[str, Any]:
    messages = load_stdout_messages(stdout)
    if not messages:
        return {}
    if summary := last_summary(messages):
        return summary
    return summarize_agent_messages(messages)


def summarize_agent_messages(messages: list[dict[str, Any]]) -> dict[str, Any]:
    summary: dict[str, Any] = {"type": "SandtableAgentSdkSummary"}
    token_cost = token_cost_from_messages(messages)
    if token_cost:
        summary["tokenCost"] = token_cost
    pipe_flow = pipe_flow_from_messages(messages)
    if pipe_flow:
        summary["pipeFlow"] = pipe_flow
    return summary
