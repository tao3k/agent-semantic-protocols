"""Guide-quality compact graph output tests."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class RealTriggerGraphOutputGuideTests(unittest.TestCase):
    def test_guide_quality_accepts_graph_entries_and_rejects_legacy_output(
        self,
    ) -> None:
        entries = (
            "entries=owner-query(O,Q=>items+tests+dependency-usage),"
            "query-deps(Q,D=>owners+imports+usage-tests),"
            "owner-tests(O=>covering-tests+test-entrypoints+fixtures),"
            "finding-frontier(F,O=>affected-owners+tests+verification-actions),"
            "feature-cfg(F2=>cfg-gates+owners+verification-surfaces)"
        )
        legacy_profiles = "profiles" + "="
        legacy_handles = "".join(["compatible", "Handles"])
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "typescript.graph-guide",
                        "language": "typescript",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                        "import json; "
                        f"entries = {entries!r}; "
                        "prime_output = 'analysis=structure nativeSyntaxFacts=skipped policyFindings=skipped\\n"
                        "aliases=G:search,O:owner,Q:query,D:dependency,T:test,F:finding,F2:feature\\n' + entries; "
                                        "decision = {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['typescript'],"
                                        "'routes': [{"
                                        "'kind': 'query',"
                                        "'argv': ['ts-harness', 'query', '--from-hook', 'direct-source-read', '--surface', 'owners,tests']"
                                        "}],"
                                        "'message': 'Use ts-harness query --from-hook direct-source-read.'"
                                        "}; "
                                        "print(json.dumps({'agentHookDecision': decision, 'searchOutput': prime_output}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "typescript",
                                        "routeKind": "query",
                                        "outputContains": [entries],
                                        "outputNotContains": [
                                            legacy_profiles,
                                            legacy_handles,
                                        ],
                                        "primeOutput": {
                                            "requiresStructureStatus": True,
                                            "requiresTypedEntryAliases": True,
                                            "entries": [entries],
                                        },
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

    def test_guide_quality_rejects_prime_output_missing_entry(self) -> None:
        entries = (
            "entries=owner-query(O,Q=>items+tests+dependency-usage),"
            "owner-tests(O=>covering-tests+test-entrypoints+fixtures)"
        )
        prime_output = (
            "analysis=structure nativeSyntaxFacts=skipped policyFindings=skipped"
        )
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "typescript.missing-prime-entry",
                        "language": "typescript",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "import json; "
                                        f"prime_output = {prime_output!r}; "
                                        "decision = {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['typescript'],"
                                        "'routes': [{'kind': 'query', 'argv': ['ts-harness', 'query']}],"
                                        "'message': 'Use ts-harness query.'"
                                        "}; "
                                        "print(json.dumps({'agentHookDecision': decision, 'searchOutput': prime_output}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "typescript",
                                        "routeKind": "query",
                                        "primeOutput": {
                                            "requiresStructureStatus": True,
                                            "entries": [entries],
                                        },
                                    }
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn(
            f"guide prime output missing entry {entries!r}",
            result.steps[0].errors,
        )

    def test_guide_quality_rejects_unknown_prime_output_profile(self) -> None:
        entries = "entries=ad-hoc-owner-map(O=>items)"
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "typescript.unknown-prime-profile",
                        "language": "typescript",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "import json; "
                                        f"entries = {entries!r}; "
                                        "decision = {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['typescript'],"
                                        "'routes': [{'kind': 'query', 'argv': ['ts-harness', 'query']}],"
                                        "'message': 'Use ts-harness query.'"
                                        "}; "
                                        "print(json.dumps({'agentHookDecision': decision, 'searchOutput': entries}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "typescript",
                                        "routeKind": "query",
                                        "primeOutput": {
                                            "entries": [entries],
                                        },
                                    }
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn(
            "guide prime output entry profile 'ad-hoc-owner-map' is not in the shared reasoning profile catalog",
            result.steps[0].errors,
        )

    def test_guide_quality_rejects_legacy_graph_profile_output(self) -> None:
        legacy_profiles = "profiles" + "="
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "typescript.bad-graph-guide",
                        "language": "typescript",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "import json; "
                                        f"bad_output = {legacy_profiles!r}; "
                                        "decision = {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['typescript'],"
                                        "'routes': [{'kind': 'query', 'argv': ['ts-harness', 'query']}],"
                                        "'message': 'Use ts-harness query.'"
                                        "}; "
                                        "print(json.dumps({'agentHookDecision': decision, 'searchOutput': bad_output}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "typescript",
                                        "routeKind": "query",
                                        "outputNotContains": [legacy_profiles],
                                    }
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn(
            f"guide output contains stale text {legacy_profiles!r}",
            result.steps[0].errors,
        )

    def test_guide_quality_rejects_graph_selector_drift(self) -> None:
        bad_output = (
            "aliases=G:search,F:reasoning-selector\n"
            "F2=finding:finding(finding(serde))!finding"
        )
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "typescript.graph-selector-drift",
                        "language": "typescript",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "import json; "
                                        f"bad_output = {bad_output!r}; "
                                        "decision = {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['typescript'],"
                                        "'routes': [{'kind': 'query', 'argv': ['ts-harness', 'query']}],"
                                        "'message': 'Use ts-harness query.'"
                                        "}; "
                                        "print(json.dumps({'agentHookDecision': decision, 'searchOutput': bad_output}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "typescript",
                                        "routeKind": "query",
                                    }
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn(
            "guide output contains graph drift text 'reasoning-selector'",
            result.steps[0].errors,
        )
        self.assertIn(
            "guide output contains graph drift text 'finding(finding('",
            result.steps[0].errors,
        )

    def test_guide_quality_rejects_entry_alias_kind_mismatch(self) -> None:
        entries = "entries=feature-cfg(F=>cfg-gates+owners+verification-surfaces)"
        prime_output = (
            "analysis=structure nativeSyntaxFacts=skipped policyFindings=skipped\n"
            "aliases=G:search,F:finding\n"
            f"{entries}"
        )
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "typescript.graph-entry-alias-mismatch",
                        "language": "typescript",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "guide",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "import json; "
                                        f"prime_output = {prime_output!r}; "
                                        "decision = {"
                                        "'reasonKind': 'raw-broad-search',"
                                        "'languageIds': ['typescript'],"
                                        "'routes': [{'kind': 'query', 'argv': ['ts-harness', 'query']}],"
                                        "'message': 'Use ts-harness query.'"
                                        "}; "
                                        "print(json.dumps({'agentHookDecision': decision, 'searchOutput': prime_output}))"
                                    ),
                                ],
                                "expect": {
                                    "guideQuality": {
                                        "reasonKind": "raw-broad-search",
                                        "languageId": "typescript",
                                        "routeKind": "query",
                                        "primeOutput": {
                                            "requiresStructureStatus": True,
                                            "requiresTypedEntryAliases": True,
                                            "entries": [entries],
                                        },
                                    }
                                },
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

        self.assertEqual("fail", result.status)
        self.assertIn(
            "guide prime output entry alias 'F' for profile 'feature-cfg' resolves to 'finding', expected 'feature'",
            result.steps[0].errors,
        )
