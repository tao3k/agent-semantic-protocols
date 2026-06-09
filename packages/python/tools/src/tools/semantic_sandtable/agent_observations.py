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
    final_answer = final_answer_from_messages(messages)
    if final_answer:
        summary["finalAnswer"] = final_answer
    return summary


def final_answer_from_messages(messages: list[dict[str, Any]]) -> dict[str, Any]:
    last_tool_message_index = -1
    last_answer_index = -1
    last_answer_text = ""
    for index, message in enumerate(messages):
        if _message_has_tool_use(message):
            last_tool_message_index = index
        text = _assistant_text(message)
        if text:
            last_answer_index = index
            last_answer_text = text
    if not last_answer_text:
        return {
            "present": False,
            "afterLastToolUse": False,
            "textBytes": 0,
            "textLineCount": 0,
            "messageIndex": None,
            "textPreview": "",
        }
    answer_bytes = len(last_answer_text.encode("utf-8"))
    return {
        "present": last_answer_index > last_tool_message_index,
        "afterLastToolUse": last_answer_index > last_tool_message_index,
        "textBytes": answer_bytes,
        "textLineCount": len(last_answer_text.splitlines()) or 1,
        "messageIndex": last_answer_index,
        "textPreview": last_answer_text[:500],
    }


def _assistant_text(message: dict[str, Any]) -> str:
    if message.get("type") == "ResultMessage":
        result = message.get("result")
        return result.strip() if isinstance(result, str) else ""
    if message.get("type") != "AssistantMessage":
        return ""
    content = message.get("content")
    if not isinstance(content, list):
        return ""
    chunks = [
        block["text"].strip()
        for block in content
        if isinstance(block, dict)
        and isinstance(block.get("text"), str)
        and block["text"].strip()
    ]
    return "\n".join(chunks).strip()


def _message_has_tool_use(message: dict[str, Any]) -> bool:
    content = message.get("content")
    if not isinstance(content, list):
        return False
    return any(_block_is_tool_use(block) for block in content)


def _block_is_tool_use(block: Any) -> bool:
    if not isinstance(block, dict):
        return False
    if block.get("type") in {"tool_use", "tool_result"}:
        return True
    return isinstance(block.get("name"), str) and isinstance(block.get("input"), dict)
