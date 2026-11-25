# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate sandtable agent step command rewriting."""

from __future__ import annotations

import unittest
from pathlib import Path

from tools.semantic_sandtable.step_runner import _resolve_step_execution


class StepRunnerAgentCommandTests(unittest.TestCase):
    def test_claude_agent_cli_step_builds_print_mode_command(self) -> None:
        step = {
            "id": "claude-print",
            "agentCli": {
                "client": "claude",
                "binary": "claude",
                "prompt": "Explain {subject}",
                "outputFormat": "stream-json",
                "inputFormat": "text",
                "includePartialMessages": True,
                "includeHookEvents": True,
                "verbose": True,
                "model": "deepseek-v4",
            },
        }

        execution = _resolve_step_execution(
            step,
            "root.claude-cli",
            "claude-print",
            {},
            {"subject": "ASP"},
        )

        self.assertIsInstance(execution, tuple)
        command, _env = execution
        self.assertEqual(
            [
                "claude",
                "-p",
                "Explain ASP",
                "--output-format",
                "stream-json",
                "--input-format",
                "text",
                "--include-partial-messages",
                "--include-hook-events",
                "--verbose",
                "--model",
                "deepseek-v4",
            ],
            command,
        )

    def test_agent_answer_required_rejects_sdk_max_turns(self) -> None:
        execution = _resolve_step_execution(
            {
                "id": "deepseek-sdk",
                "agentSdk": {
                    "client": "deepseek",
                    "prompt": "Explain ASP",
                    "outputFormat": "stream-json",
                    "maxTurns": 9,
                },
                "expect": {"agentAnswer": {"required": True}},
            },
            "root.deepseek-sdk",
            "deepseek-sdk",
            {},
            {},
            repo_root=Path("/workspace"),
        )

        self.assertNotIsInstance(execution, tuple)
        self.assertEqual(
            [
                "step.agentSdk.maxTurns cannot be used when expect.agentAnswer.required is true; use timeoutSeconds instead"
            ],
            execution.errors,
        )

    def test_claude_agent_sdk_client_is_rejected(self) -> None:
        execution = _resolve_step_execution(
            {
                "id": "claude-sdk",
                "agentSdk": {
                    "client": "claude",
                    "prompt": "Explain ASP",
                    "outputFormat": "summary-json",
                },
            },
            "root.claude-sdk",
            "claude-sdk",
            {},
            {},
            repo_root=Path("/workspace"),
        )

        self.assertNotIsInstance(execution, tuple)
        self.assertEqual(
            ["step.agentSdk.client must be 'deepseek' or 'openai-compatible'"],
            execution.errors,
        )

    def test_step_cannot_define_command_and_agent_cli(self) -> None:
        execution = _resolve_step_execution(
            {
                "id": "ambiguous",
                "command": ["true"],
                "agentCli": {
                    "client": "claude",
                    "binary": "claude",
                    "prompt": "hello",
                    "outputFormat": "text",
                },
            },
            "root.claude-cli",
            "ambiguous",
            {},
            {},
        )

        self.assertEqual("fail", execution.status)
        self.assertEqual(
            ["step must define exactly one of command, agentCli, or agentSdk"],
            execution.errors,
        )

    def test_step_cannot_define_agent_cli_and_agent_sdk(self) -> None:
        execution = _resolve_step_execution(
            {
                "id": "ambiguous",
                "agentCli": {
                    "client": "claude",
                    "binary": "claude",
                    "prompt": "hello",
                    "outputFormat": "text",
                },
                "agentSdk": {
                    "client": "claude",
                    "prompt": "hello",
                    "outputFormat": "text",
                },
            },
            "root.claude-sdk",
            "ambiguous",
            {},
            {},
        )

        self.assertEqual("fail", execution.status)
        self.assertEqual(
            ["step must define exactly one of command, agentCli, or agentSdk"],
            execution.errors,
        )


if __name__ == "__main__":
    unittest.main()
