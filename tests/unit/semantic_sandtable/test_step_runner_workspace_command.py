"""Validate sandtable workspace command rewriting."""

from __future__ import annotations

import unittest
from pathlib import Path
from unittest.mock import patch

from tools.semantic_sandtable.step_runner import (
    _resolve_step_execution,
    _workspace_dev_command,
)


class StepRunnerWorkspaceCommandTests(unittest.TestCase):
    def test_protocol_hook_command_uses_workspace_binary_and_activation(self) -> None:
        repo_root = Path("/workspace")

        command = _workspace_dev_command(
            repo_root,
            ["asp", "hook", "pre-tool", "--client", "codex"],
        )

        self.assertEqual(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                "/workspace/crates/agent-semantic-protocol/Cargo.toml",
                "--",
                "hook",
                "pre-tool",
                "--client",
                "codex",
                "--activation",
                "/workspace/.cache/agent-semantic-protocol/hooks/activation.json",
            ],
            [str(part) for part in command],
        )

    def test_protocol_non_hook_command_uses_workspace_binary_without_activation(
        self,
    ) -> None:
        repo_root = Path("/workspace")

        command = _workspace_dev_command(
            repo_root,
            ["asp", "graph", "render", "--packet", "-"],
        )

        self.assertEqual(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                "/workspace/crates/agent-semantic-protocol/Cargo.toml",
                "--",
                "graph",
                "render",
                "--packet",
                "-",
            ],
            [str(part) for part in command],
        )

    def test_legacy_hook_command_is_not_rewritten(self) -> None:
        command = ["agent-semantic-hook", "hook", "--client", "codex", "pre-tool"]

        self.assertEqual(command, _workspace_dev_command(Path("/workspace"), command))

    def test_old_protocol_command_is_not_rewritten(self) -> None:
        command = ["agent-semantic-protocol", "hook", "pre-tool", "--client", "codex"]

        self.assertEqual(command, _workspace_dev_command(Path("/workspace"), command))

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
            ["step must define exactly one of command or agentCli"],
            execution.errors,
        )

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


if __name__ == "__main__":
    unittest.main()
