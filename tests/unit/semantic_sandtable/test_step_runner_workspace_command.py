"""Validate sandtable workspace command rewriting."""

from __future__ import annotations

import unittest
from pathlib import Path

from tools.semantic_sandtable.step_runner import _workspace_dev_command


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


if __name__ == "__main__":
    unittest.main()
