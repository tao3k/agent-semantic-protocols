"""Validate agent token budget expectation warnings."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from tools.semantic_sandtable.expectations import validate_step
from tools.semantic_sandtable.models import StepResult


def test_agent_token_budget_warnings_use_observed_token_cost() -> None:
    result = _step_result(
        observations={
            "tokenCost": {
                "inputTokens": 100,
                "outputTokens": 30,
                "cacheReadInputTokens": 700,
                "totalTokens": 830,
                "costUsd": 0.5,
            }
        }
    )

    validate_step(
        {
            "expect": {
                "maxAgentInputTokensWarn": 80,
                "maxAgentOutputTokensWarn": 20,
                "maxAgentCacheReadInputTokensWarn": 600,
                "maxAgentTotalTokensWarn": 800,
                "maxAgentCostUsdWarn": 0.25,
            }
        },
        result,
        "done",
        "",
        Path("."),
    )

    assert result.errors == []
    assert (
        "tokenCost.inputTokens=100 exceeds maxAgentInputTokensWarn=80"
        in result.warnings
    )
    assert (
        "tokenCost.outputTokens=30 exceeds maxAgentOutputTokensWarn=20"
        in result.warnings
    )
    assert (
        "tokenCost.cacheReadInputTokens=700 exceeds "
        "maxAgentCacheReadInputTokensWarn=600"
    ) in result.warnings
    assert (
        "tokenCost.totalTokens=830 exceeds maxAgentTotalTokensWarn=800"
        in result.warnings
    )
    assert (
        "tokenCost.costUsd=0.500000 exceeds maxAgentCostUsdWarn=0.250000"
        in result.warnings
    )


def test_agent_token_budget_warnings_report_missing_token_cost() -> None:
    result = _step_result()

    validate_step(
        {
            "expect": {
                "maxAgentTotalTokensWarn": 800,
                "maxAgentCostUsdWarn": 0.25,
            }
        },
        result,
        "done",
        "",
        Path("."),
    )

    assert result.errors == []
    assert (
        "tokenCost totalTokens missing for maxAgentTotalTokensWarn"
        in result.warnings
    )
    assert "tokenCost costUsd missing for maxAgentCostUsdWarn" in result.warnings


def _step_result(observations: dict[str, Any] | None = None) -> StepResult:
    return StepResult(
        scenario_id="rust.live",
        step_id="claude",
        command=["claude"],
        status="pass",
        exit_code=0,
        elapsed_ms=10,
        stdout_lines=1,
        stderr_lines=0,
        stdout_bytes=4,
        stderr_bytes=0,
        observations=observations or {},
    )
