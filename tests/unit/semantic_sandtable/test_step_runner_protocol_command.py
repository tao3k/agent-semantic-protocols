"""Validate sandtable workspace protocol command rewriting."""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tools.semantic_sandtable.step_runner import _workspace_dev_command


class StepRunnerProtocolCommandTests(unittest.TestCase):
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

    def test_protocol_command_prefers_built_workspace_binary(self) -> None:
        with TemporaryDirectory() as directory:
            repo_root = Path(directory)
            binary = repo_root / "target/debug/asp"
            binary.parent.mkdir(parents=True)
            binary.write_text("#!/bin/sh\n", encoding="utf-8")

            command = _workspace_dev_command(
                repo_root,
                ["asp", "rust", "search", "lexical", "codeql"],
            )

        self.assertEqual(
            [
                str(binary.resolve()),
                "rust",
                "search",
                "lexical",
                "codeql",
            ],
            [str(part) for part in command],
        )

    def test_protocol_hook_binary_command_still_appends_activation(self) -> None:
        with TemporaryDirectory() as directory:
            repo_root = Path(directory)
            binary = repo_root / ".bin/asp"
            binary.parent.mkdir(parents=True)
            binary.write_text("#!/bin/sh\n", encoding="utf-8")

            command = _workspace_dev_command(
                repo_root,
                ["asp", "hook", "pre-tool", "--client", "codex"],
            )

        self.assertEqual(
            [
                str(binary.resolve()),
                "hook",
                "pre-tool",
                "--client",
                "codex",
                "--activation",
                str(repo_root / ".cache/agent-semantic-protocol/hooks/activation.json"),
            ],
            [str(part) for part in command],
        )

    def test_direct_language_harness_commands_are_not_python_rewritten(self) -> None:
        commands = [
            ["rs-harness", "search", "prime", "--workspace", "."],
            ["ts-harness", "search", "prime", "--workspace", "."],
            ["asp-julia-harness", "search", "prime", "--workspace", "."],
            ["py-harness", "search", "prime", "--workspace", "."],
        ]

        for command in commands:
            with self.subTest(command=command[0]):
                self.assertEqual(
                    command,
                    _workspace_dev_command(Path("/workspace"), command),
                )

    def test_python_protocol_command_uses_workspace_protocol_binary(self) -> None:
        with TemporaryDirectory() as directory:
            repo_root = Path(directory)
            binary = repo_root / ".bin" / "asp"
            binary.parent.mkdir(parents=True)
            binary.write_text("#!/bin/sh\n", encoding="utf-8")

            command = _workspace_dev_command(
                repo_root,
                ["asp", "python", "search", "prime", "--workspace", "."],
            )

        self.assertEqual(
            [
                str(binary.resolve()),
                "python",
                "search",
                "prime",
                "--workspace",
                ".",
            ],
            [str(part) for part in command],
        )

    def test_retired_hook_command_is_not_rewritten(self) -> None:
        command = ["agent-semantic-hook", "hook", "--client", "codex", "pre-tool"]

        self.assertEqual(command, _workspace_dev_command(Path("/workspace"), command))

    def test_old_protocol_command_is_not_rewritten(self) -> None:
        command = ["agent-semantic-protocol", "hook", "pre-tool", "--client", "codex"]

        self.assertEqual(command, _workspace_dev_command(Path("/workspace"), command))


if __name__ == "__main__":
    unittest.main()
