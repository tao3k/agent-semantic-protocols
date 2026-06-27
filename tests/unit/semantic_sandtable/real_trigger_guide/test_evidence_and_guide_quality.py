"""Real-trigger evidence and guide-quality behavior tests."""

from __future__ import annotations

import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.scenario_runner import run_scenario


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[4]


class RealTriggerEvidenceGuideTests(unittest.TestCase):
    def test_real_trigger_evidence_and_guide_quality_are_reported(self) -> None:
        repo_root = _PROTOCOL_REPO_ROOT
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = Path(tmp) / "receipt.json"
            receipt_path.write_text(
                json.dumps(
                    {
                        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
                        "schemaVersion": "1",
                        "scenarioId": "rust.real-trigger",
                        "language": "rust",
                        "project": {"name": "tokio", "source": "fixture"},
                        "intent": "Add async IO feature behavior before editing",
                        "editBoundary": "before-edit",
                        "commands": [
                            {
                                "id": "guide",
                                "kind": "hook-deny",
                                "argv": ["rs-harness", "search", "ingest", "."],
                                "stdinShape": "hook-payload",
                                "decisionReasonKind": "raw-broad-search",
                                "routeKind": "ingest",
                                "metrics": {
                                    "elapsedMs": 1,
                                    "stdoutBytes": 10,
                                    "stderrBytes": 0,
                                },
                            }
                        ],
                        "summary": {
                            "commandCount": 1,
                            "stdoutBytes": 10,
                            "stderrBytes": 0,
                            "elapsedMs": 1,
                            "repeatedSearches": 1,
                            "tokenDelta": "lower",
                        },
                    }
                ),
                encoding="utf-8",
            )
            scenario_path = Path(tmp) / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.real-trigger",
                        "language": "rust",
                        "workdir": ".",
                        "coverage": ["real-trigger", "codex-hooks"],
                        "evidence": {
                            "source": "real-trigger",
                            "intent": "Add async IO feature behavior before editing",
                            "receiptPath": "receipt.json",
                            "editBoundary": "before-edit",
                            "metrics": {
                                "recordedCommandCount": 3,
                                "repeatedSearches": 1,
                                "tokenDelta": "lower",
                            },
                            "querySetOpportunities": [
                                {
                                    "view": "owner",
                                    "queries": 3,
                                    "saveCommands": 2,
                                    "selector": "exact-set",
                                    "reason": "same owner axis",
                                }
                            ],
                            "findings": [
                                {
                                    "kind": "prime-quality",
                                    "severity": "info",
                                    "message": "prime selected the async IO axis",
                                }
                            ],
                        },
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "import json; "
                                        "print(json.dumps({"
                                        "'agentHookDecision': {"
                                        "'schemaId': 'agent.semantic-protocols.hook.decision',"
                                        "'schemaVersion': '1',"
                                        "'protocolId': 'agent.semantic-protocols.hook',"
                                        "'protocolVersion': '1',"
                                        "'platform': 'codex',"
                                        "'event': 'pre-tool',"
                                        "'decision': 'deny',"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['rust'],"
                                        "'subject': {'toolName': 'Bash', 'command': 'external candidate search'},"
                                        "'routes': [{"
                                        "'languageId': 'rust',"
                                        "'providerId': 'rs-harness',"
                                        "'binary': 'rs-harness',"
                                        "'kind': 'ingest',"
                                        "'argv': ['rs-harness', 'search', 'ingest', 'items', 'tests', '.'],"
                                        "'stdinMode': 'pipe-candidates'"
                                        "}],"
                                        "'message': 'Pipe candidates into rs-harness search ingest.'"
                                        "}}))"
                                    ),
                                ],
                                "expect": {
                                    "stdoutJsonSchemaAt": {
                                        "agentHookDecision": (
                                            "schemas/semantic-agent-hook-decision.v1.schema.json"
                                        )
                                    },
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "rust",
                                        "routeKind": "ingest",
                                        "commandContains": [
                                            "rs-harness",
                                            "search",
                                            "ingest",
                                        ],
                                        "requiresIngestPipe": True,
                                        "sourceLeakNotContains": ["pub mod"],
                                    },
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)
            stdout = StringIO()
            with redirect_stdout(stdout):
                exit_code = main(["--repo-root", str(repo_root), str(scenario_path)])

        self.assertEqual("pass", result.status)
        self.assertEqual(0, exit_code)
        self.assertIn("[sandtable-flow] scenario=rust.real-trigger", stdout.getvalue())
        self.assertIn("|merge view=owner queries=3", stdout.getvalue())
        self.assertIn("|finding kind=prime-quality", stdout.getvalue())
