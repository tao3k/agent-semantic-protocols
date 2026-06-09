"""Validate liveAgent to Claude SDK step lowering."""

from __future__ import annotations

from pathlib import Path

from tools.semantic_sandtable.models import ScenarioResult
from tools.semantic_sandtable.scenario_runner import _resolve_scenario_steps
from tools.semantic_sandtable.step_runner import _resolve_step_execution


def test_live_agent_scenario_derives_single_repo_claude_step() -> None:
    scenario = {
        "id": "typescript.effect-live",
        "language": "typescript",
        "workdir": ".",
        "liveAgent": {
            "client": "claude",
            "outputFormat": "stream-json",
            "includeHookEvents": True,
            "verbose": True,
            "useRepoClaudeSettings": True,
            "timeoutSeconds": 120,
            "expect": {
                "agentAnswer": {"required": True},
                "pipeFlow": {"maxAspCommands": 3},
            },
        },
        "evidence": {
            "source": "real-trigger",
            "deepQuestionCases": [
                {
                    "id": "effect-question",
                    "question": "Where should Effect concurrency behavior be located?",
                    "stepIds": ["effect-question"],
                    "queryTerms": ["Effect", "concurrency", "Fiber"],
                    "audit": {
                        "maxAspCommands": 3,
                        "maxSearchCommands": 2,
                        "maxQueryCommands": 1,
                        "maxRepeatedCommands": 0,
                        "requiresGraphSignals": True,
                    },
                }
            ],
        },
    }
    result = ScenarioResult(
        scenario_id="typescript.effect-live",
        language="typescript",
        path=Path("sandtables/typescript/effect-live.json"),
        status="pass",
        workdir=Path("/workspace"),
    )

    steps = _resolve_scenario_steps(scenario, result)

    assert result.status == "pass"
    assert result.errors == []
    assert isinstance(steps, list)
    assert len(steps) == 1
    step = steps[0]
    assert step["id"] == "effect-question"
    assert step["kind"] == "agent-sdk"
    assert "command" not in step
    assert step["timeoutSeconds"] == 120
    agent_sdk = step["agentSdk"]
    assert agent_sdk["prompt"] == "Where should Effect concurrency behavior be located?"
    assert agent_sdk["useRepoClaudeSettings"] is True
    assert "allowedTools" not in agent_sdk
    assert agent_sdk["requireAspBashCommands"] is True
    assert agent_sdk["maxAspBashCommands"] == 3

    execution = _resolve_step_execution(
        step,
        "typescript.effect-live",
        "effect-question",
        {},
        {},
        repo_root=Path("/workspace"),
    )
    assert isinstance(execution, tuple)
    command, step_env = execution
    assert "ASP_HOOK_PROJECT_ROOT" not in step_env
    assert "--settings" in command
    assert "/workspace/.claude/settings.json" in command
    assert "--allowed-tool" not in command
    assert "--require-asp-bash-commands" in command
    assert "--max-asp-bash-commands" in command
    assert "3" in command
