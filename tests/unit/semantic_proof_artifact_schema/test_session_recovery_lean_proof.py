"""Verify the Lean state-machine proof for agent session recovery."""

import shutil
import subprocess
import unittest

from .support import REPO_ROOT


class AgentSessionRecoveryLeanFileTests(unittest.TestCase):
    def test_agent_session_recovery_lean_file_checks(self) -> None:
        lean_bin = shutil.which("lean")
        self.assertIsNotNone(
            lean_bin,
            "Lean binary is required to verify agent session recovery obligations",
        )

        proof_path = (
            REPO_ROOT
            / "tests"
            / "fixtures"
            / "agent_session_recovery"
            / "agent_session_recovery.lean"
        )
        result = subprocess.run(
            [lean_bin, str(proof_path)],
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(
            result.returncode,
            0,
            f"Lean proof failed for {proof_path}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
