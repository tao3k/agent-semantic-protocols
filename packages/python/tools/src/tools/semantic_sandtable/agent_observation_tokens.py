"""Token and cost aggregation for agent observation summaries."""

from __future__ import annotations

from typing import Any

from .agent_observation_json import float_value, int_value, walk

_TOKEN_FIELDS = {
    "input_tokens": "inputTokens",
    "inputTokens": "inputTokens",
    "output_tokens": "outputTokens",
    "outputTokens": "outputTokens",
    "cache_creation_input_tokens": "cacheCreationInputTokens",
    "cacheCreationInputTokens": "cacheCreationInputTokens",
    "cache_read_input_tokens": "cacheReadInputTokens",
    "cacheReadInputTokens": "cacheReadInputTokens",
    "cache_write_input_tokens": "cacheWriteInputTokens",
    "cacheWriteInputTokens": "cacheWriteInputTokens",
}
_COST_FIELDS = {"costUsd", "cost_usd", "totalCostUsd", "total_cost_usd"}


def token_cost_from_messages(messages: list[dict[str, Any]]) -> dict[str, Any]:
    totals = {field: 0 for field in set(_TOKEN_FIELDS.values())}
    usage_records = 0
    costs: list[float] = []
    for value in walk(messages):
        if not isinstance(value, dict):
            continue
        usage_records += _add_usage_totals(value, totals)
        costs.extend(_cost_values(value))
    return _compact_token_cost(totals, usage_records, costs)


def _add_usage_totals(value: dict[str, Any], totals: dict[str, int]) -> int:
    if not _looks_like_usage(value):
        return 0
    for source, target in _TOKEN_FIELDS.items():
        amount = int_value(value.get(source))
        if amount is not None:
            totals[target] += amount
    return 1


def _cost_values(value: dict[str, Any]) -> list[float]:
    costs = []
    for cost_field in _COST_FIELDS:
        cost = float_value(value.get(cost_field))
        if cost is not None:
            costs.append(cost)
    return costs


def _compact_token_cost(
    totals: dict[str, int],
    usage_records: int,
    costs: list[float],
) -> dict[str, Any]:
    compact: dict[str, Any] = {
        key: value for key, value in sorted(totals.items()) if value
    }
    if not compact and not costs:
        return {}
    compact["totalTokens"] = sum(compact.values())
    compact["usageRecords"] = usage_records
    if costs:
        compact["costUsd"] = max(costs)
    compact["source"] = "claude-sdk-stream"
    return compact


def _looks_like_usage(value: dict[str, Any]) -> bool:
    return any(field in value for field in _TOKEN_FIELDS)
