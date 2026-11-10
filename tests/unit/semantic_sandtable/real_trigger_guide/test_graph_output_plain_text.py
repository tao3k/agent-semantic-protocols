# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Guide-quality plain text graph output tests."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class RealTriggerPlainTextGraphGuideTests(unittest.TestCase):
    def test_guide_quality_accepts_plain_text_output_without_decision(self) -> None:
        output = (
            "[guide] lang=rust provider=asp-rust protocol=guide.v1\n"
            "|catalog reasoningProfiles=owner-query,query-deps,owner-tests,finding-frontier,feature-cfg "
            "entries=owner-query,query-deps,owner-tests,finding-frontier,feature-cfg "
            "routes=path,read-frontier\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.plain-guide-quality",
                        "language": "rust",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    "python",
                                    "-c",
                                    f"print({output!r}, end='')",
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "outputContains": ["|catalog reasoningProfiles="],
                                        "outputNotContains": ["profiles=", "owner-items"],
                                    }
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("pass", result.status)
