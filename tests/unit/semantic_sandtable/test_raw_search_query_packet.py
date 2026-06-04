"""Validate raw-search query packet coverage in sandtable receipts."""

from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


def _cargo_run_command(*args: str) -> list[str]:
    cargo_prefix = (
        ["cargo"]
        if shutil.which("cargo") is not None
        else ["direnv", "exec", ".", "cargo"]
    )
    return [
        *cargo_prefix,
        "run",
        "--quiet",
        "--manifest-path",
        "languages/rust-lang-project-harness/Cargo.toml",
        "--features",
        "cli,search",
        "--bin",
        "rs-harness",
        "--",
        *args,
    ]


class RawSearchQueryPacketTests(unittest.TestCase):
    def test_raw_search_owner_query_names_only_and_json_packet(self) -> None:
        repo_root = _PROTOCOL_REPO_ROOT
        with tempfile.TemporaryDirectory() as tmp:
            scenario_path = Path(tmp) / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.raw-search-query-packet",
                        "language": "rust",
                        "workdir": ".",
                        "coverage": ["real-trigger", "search-flow"],
                        "evidence": {
                            "source": "real-trigger",
                            "intent": "repair raw_search owner-local symbol miss",
                            "editBoundary": "before-edit",
                            "metrics": {
                                "recordedCommandCount": 22,
                                "repeatedSearches": 18,
                                "tokenDelta": "lower",
                            },
                            "querySetOpportunities": [
                                {
                                    "view": "owner",
                                    "queries": 22,
                                    "saveCommands": 20,
                                    "selector": "exact-set",
                                    "reason": "same owner item axis via query",
                                }
                            ],
                            "findings": [
                                {
                                    "kind": "owner-local-query-repair",
                                    "severity": "info",
                                    "message": "miss emits parser-owned candidates",
                                }
                            ],
                        },
                        "steps": [
                            {
                                "id": "miss-json",
                                "command": _cargo_run_command(
                                    "query",
                                    "crates/agent-semantic-hook/src/command/raw_search.rs",
                                    "--term",
                                    "parse_ripgrep_scope",
                                    "--names-only",
                                    "--json",
                                    "crates/agent-semantic-hook",
                                ),
                                "expect": {
                                    "stdoutJsonSchema": (
                                        "schemas/semantic-query-packet.v1.schema.json"
                                    ),
                                    "stdoutJsonEquals": {
                                        "schemaId": (
                                            "agent.semantic-protocols.semantic-query-packet"
                                        ),
                                        "outputMode": "names",
                                        "queryCoverage.0.candidateNames.0": (
                                            "parse_ripgrep_like"
                                        ),
                                        "candidateItems.0.name": "parse_ripgrep_like",
                                    },
                                },
                            },
                            {
                                "id": "prefix-names-only",
                                "command": _cargo_run_command(
                                    "query",
                                    "crates/agent-semantic-hook/src/command/raw_search.rs",
                                    "--term",
                                    "parse_",
                                    "--names-only",
                                    "crates/agent-semantic-hook",
                                ),
                                "expect": {
                                    "stdoutContains": [
                                        "output=names",
                                        "|item parse_ripgrep_like kind=fn",
                                    ],
                                    "stdoutNotContains": ["|code path="],
                                },
                            },
                            {
                                "id": "query-set-compact-frontier",
                                "command": _cargo_run_command(
                                    "search",
                                    "fzf",
                                    "--query-set",
                                    "DecisionRouteKind::Read",
                                    "--query-set",
                                    "window_set",
                                    "--query-set",
                                    "direct-source-read",
                                    "--view",
                                    "seeds",
                                    "crates/agent-semantic-hook",
                                ),
                                "expect": {
                                    "stdoutContains": [
                                        "[search-fzf]",
                                        "alias: graph:{G=search",
                                        "Q=query:term(DecisionRouteKind::Read,window_set,direct-source-read)!fzf",
                                        "test:path(tests/unit/classifier/routes/direct_read/exact.rs)!tests",
                                        "G>{Q:matches",
                                        "frontier=Q.fzf",
                                    ],
                                },
                            },
                        ],
                    }
                ),
                encoding="utf-8",
            )
            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("pass", result.status, result.errors)
        self.assertEqual(3, len(result.steps))
        self.assertLess(
            len(result.steps),
            result.evidence["metrics"]["recordedCommandCount"],
        )
        self.assertGreater(
            result.evidence["metrics"]["repeatedSearches"],
            0,
        )
        self.assertGreater(sum(step.elapsed_ms for step in result.steps), 0)
        self.assertGreater(sum(step.stdout_bytes for step in result.steps), 0)
