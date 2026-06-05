"""Validate semantic sandtable environment wiring for provider subprocesses."""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.semantic_sandtable.utils import build_env


class SandtableEnvTests(unittest.TestCase):
    def test_build_env_uses_workspace_protocol_renderer(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            protocol_bin = repo_root / ".bin" / "asp"
            protocol_bin.parent.mkdir()
            protocol_bin.write_text("#!/bin/sh\n", encoding="utf-8")

            with patch.dict(os.environ, {}, clear=True):
                env = build_env({}, repo_root=repo_root)

        self.assertEqual(
            str(protocol_bin.resolve()),
            env["SEMANTIC_AGENT_PROTOCOL_BIN"],
        )

    def test_build_env_prefers_target_protocol_renderer(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            protocol_bin = repo_root / ".bin" / "asp"
            target_bin = repo_root / "target" / "debug" / "asp"
            protocol_bin.parent.mkdir()
            target_bin.parent.mkdir(parents=True)
            protocol_bin.write_text("#!/bin/sh\n", encoding="utf-8")
            target_bin.write_text("#!/bin/sh\n", encoding="utf-8")

            with patch.dict(os.environ, {}, clear=True):
                env = build_env({}, repo_root=repo_root)

        self.assertEqual(
            str(target_bin.resolve()),
            env["SEMANTIC_AGENT_PROTOCOL_BIN"],
        )

    def test_build_env_preserves_explicit_protocol_renderer(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            protocol_bin = repo_root / ".bin" / "asp"
            protocol_bin.parent.mkdir()
            protocol_bin.write_text("#!/bin/sh\n", encoding="utf-8")

            with patch.dict(os.environ, {}, clear=True):
                env = build_env(
                    {"SEMANTIC_AGENT_PROTOCOL_BIN": "/custom/semantic-agent-protocol"},
                    repo_root=repo_root,
                )

        self.assertEqual(
            "/custom/semantic-agent-protocol",
            env["SEMANTIC_AGENT_PROTOCOL_BIN"],
        )
