from __future__ import annotations

import json
import sys
from pathlib import Path

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.agent_observation_tokens import token_cost_from_messages
from tools.semantic_sandtable.step_agent_sdk import resolve_agent_sdk_step


def test_deepseek_agent_sdk_builds_openai_compatible_runner_command() -> None:
    resolved = resolve_agent_sdk_step(
        {
            "agentSdk": {
                "client": "deepseek",
                "prompt": "answer with ok",
                "outputFormat": "summary-json",
                "model": "deepseek-chat",
                "baseUrl": "https://api.deepseek.com",
                "apiKeyEnv": "DEEPSEEK_API_KEY",
                "requireLiveUsage": True,
            }
        },
        "scenario",
        "step",
        {},
        {},
        Path("."),
    )

    assert isinstance(resolved, tuple)
    command, env = resolved
    assert command == [
        sys.executable,
        "-m",
        "tools.semantic_sandtable.openai_compatible_runner",
        "--provider",
        "deepseek",
        "--prompt",
        "answer with ok",
        "--output-format",
        "summary-json",
        "--model",
        "deepseek-chat",
        "--base-url",
        "https://api.deepseek.com",
        "--api-key-env",
        "DEEPSEEK_API_KEY",
        "--require-live-usage",
    ]
    assert env == {}


def test_sandtable_schema_accepts_deepseek_agent_sdk_step() -> None:
    schema = json.loads(
        Path("schemas/semantic-sandtable-scenario.v1.schema.json").read_text()
    )
    scenario = {
        "id": "python.deepseek-live-smoke",
        "language": "python",
        "workdir": ".",
        "steps": [
            {
                "id": "deepseek-ok",
                "timeoutSeconds": 120,
                "agentSdk": {
                    "client": "deepseek",
                    "prompt": "Return exactly OK.",
                    "outputFormat": "summary-json",
                    "model": "deepseek-chat",
                    "baseUrl": "https://api.deepseek.com",
                    "apiKeyEnv": "DEEPSEEK_API_KEY",
                    "requireLiveUsage": True,
                },
            }
        ],
    }

    errors = sorted(
        Draft202012Validator(schema).iter_errors(scenario),
        key=lambda error: list(error.path),
    )
    assert errors == []


def test_token_cost_from_messages_preserves_openai_compatible_origin() -> None:
    token_cost = token_cost_from_messages(
        [
            {
                "tokenCost": {
                    "inputTokens": 7,
                    "outputTokens": 5,
                    "source": "openai-compatible-live",
                    "provider": "deepseek",
                    "model": "deepseek-chat",
                }
            }
        ]
    )

    assert token_cost["totalTokens"] == 12
    assert token_cost["source"] == "openai-compatible-live"
    assert token_cost["providers"] == ["deepseek"]
    assert token_cost["models"] == ["deepseek-chat"]
