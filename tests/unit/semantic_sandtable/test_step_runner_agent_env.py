# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate sandtable agent step environment resolution."""

from __future__ import annotations

import unittest
from unittest.mock import patch

from tools.semantic_sandtable.step_runner import _resolve_step_execution


class StepRunnerAgentEnvTests(unittest.TestCase):
    def test_agent_cli_required_env_must_be_resolved_before_spawn(self) -> None:
        with patch.dict("os.environ", {}, clear=True):
            execution = _resolve_step_execution(
                {
                    "id": "deepseek-missing-token",
                    "agentCli": {
                        "client": "claude",
                        "binary": "claude",
                        "prompt": "hello",
                        "outputFormat": "stream-json",
                        "env": {
                            "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
                            "ANTHROPIC_AUTH_TOKEN": "${DEEPSEEK_API_KEY}",
                        },
                        "requiredEnv": ["ANTHROPIC_AUTH_TOKEN"],
                    },
                },
                "root.claude-cli",
                "deepseek-missing-token",
                {},
                {},
            )

        self.assertEqual("fail", execution.status)
        self.assertEqual(
            ["step.agentCli.requiredEnv unresolved: ANTHROPIC_AUTH_TOKEN"],
            execution.errors,
        )

    def test_agent_cli_env_expands_from_base_environment(self) -> None:
        execution = _resolve_step_execution(
            {
                "id": "deepseek-token",
                "agentCli": {
                    "client": "claude",
                    "binary": "claude",
                    "prompt": "hello",
                    "outputFormat": "stream-json",
                    "env": {
                        "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
                        "ANTHROPIC_AUTH_TOKEN": "${DEEPSEEK_API_KEY}",
                    },
                    "requiredEnv": ["ANTHROPIC_AUTH_TOKEN"],
                },
            },
            "root.claude-cli",
            "deepseek-token",
            {"DEEPSEEK_API_KEY": "fake-token"},
            {},
        )

        self.assertIsInstance(execution, tuple)
        _command, env = execution
        self.assertEqual("fake-token", env["ANTHROPIC_AUTH_TOKEN"])

    def test_agent_sdk_required_env_must_be_resolved_before_spawn(self) -> None:
        with patch.dict("os.environ", {}, clear=True):
            execution = _resolve_step_execution(
                {
                    "id": "claude-sdk-missing-token",
                    "agentSdk": {
                        "client": "claude",
                        "prompt": "hello",
                        "outputFormat": "stream-json",
                        "env": {
                            "ANTHROPIC_AUTH_TOKEN": "${ANTHROPIC_AUTH_TOKEN}",
                        },
                        "requiredEnv": ["ANTHROPIC_AUTH_TOKEN"],
                    },
                },
                "root.claude-sdk",
                "claude-sdk-missing-token",
                {},
                {},
            )

        self.assertEqual("fail", execution.status)
        self.assertEqual(
            ["step.agentSdk.requiredEnv unresolved: ANTHROPIC_AUTH_TOKEN"],
            execution.errors,
        )


if __name__ == "__main__":
    unittest.main()
