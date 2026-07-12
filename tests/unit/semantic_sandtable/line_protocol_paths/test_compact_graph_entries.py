"""Validate compact graph entry selector contracts in line protocol output."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from tools.semantic_sandtable.scenario_runner import run_scenario


class LineProtocolCompactGraphEntriesTests(unittest.TestCase):
    def test_line_protocol_accepts_schema_owned_compact_graph_entries(self) -> None:
        result = _run_entry_scenario(
            "python.compact-graph-entries-line-protocol",
            (
                "print('[search-query] q=async selector=src/lib.rs alg=query-set')\n"
                "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                "print('aliases: graph:{G=search,O=owner,Q=query,D=dependency}')\n"
                "print('O=owner:path(src/lib.rs)!owner;Q=query:term(async)!lexical;D=dependency:pkg(tokio)!deps')\n"
                "print('G>{O:selects,Q:matches,D:uses}')\n"
                "print('rank=O,Q,D frontier=O.owner,Q.lexical,D.deps')\n"
                "print('entries=owner-query(O,Q=>items+tests),query-deps(Q,D=>owners+imports),owner-tests(O=>covering-tests+fixtures)')"
            ),
        )

        self.assertEqual("pass", result.status)

    def test_line_protocol_accepts_optional_finding_frontier_owner(self) -> None:
        result = _run_entry_scenario(
            "python.compact-graph-finding-frontier-entry",
            (
                "print('[search-reasoning] q=finding-frontier selector=finding=py-policy alg=seed-frontier')\n"
                "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                "print('aliases: graph:{G=search,F=finding,O=owner}')\n"
                "print('F=finding:finding(py-policy)!finding;O=owner:path(src/lib.rs)!owner')\n"
                "print('G>{F:flags,O:selects}')\n"
                "print('rank=F,O frontier=F.finding,O.owner')\n"
                "print('entries=finding-frontier(F,O=>affected-owners+tests+verification-actions)')"
            ),
        )

        self.assertEqual("pass", result.status)

    def test_line_protocol_accepts_compact_graph_omit_reason_metadata(self) -> None:
        result = _run_entry_scenario(
            "python.compact-graph-omit-reason-metadata",
            (
                "print('[search-prime] root=. mode=fast')\n"
                "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                "print('aliases: graph:{G=search,Q=query,O=owner}')\n"
                "print('Q=query:term(prime)!query;O=owner:path(src/lib.rs)!owner')\n"
                "print('G>{Q:matches,O:selects}')\n"
                "print('rank=Q,O frontier=Q.query,O.owner')\n"
                "print('entries=owner-query(O,Q=>items+tests)')\n"
                "print('omit=items,blocks,code,full-json reason=fast-seeds-frontier')"
            ),
        )

        self.assertEqual("pass", result.status)

    def test_line_protocol_accepts_hyphenated_alias_node_kinds(self) -> None:
        result = _run_entry_scenario(
            "python.compact-graph-hyphenated-alias-node-kinds",
            (
                "print('[search-dependency] q=walkdir alg=dependency-frontier')\n"
                "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                "print('aliases: graph:{G=search,U=doc-use,C=crate-source,O=owner}')\n"
                "print('U=doc-use:path(README.md)!docs;C=crate-source:pkg(walkdir)!deps;O=owner:path(src/lib.rs)!owner')\n"
                "print('G>{U:documents,C:uses,O:selects}')\n"
                "print('rank=U,C,O frontier=U.docs,C.deps,O.owner')"
            ),
        )

        self.assertEqual("pass", result.status)

    def test_line_protocol_rejects_entry_alias_missing_from_graph(self) -> None:
        result = _run_entry_scenario(
            "python.compact-graph-entry-missing-alias",
            (
                "print('[search-query] q=async selector=src/lib.rs alg=query-set')\n"
                "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                "print('aliases: graph:{G=search,O=owner}')\n"
                "print('O=owner:path(src/lib.rs)!owner')\n"
                "print('G>{O:selects}')\n"
                "print('rank=O frontier=O.owner')\n"
                "print('entries=owner-tests(X=>covering-tests)')"
            ),
        )

        self.assertEqual("fail", result.status)
        self.assertIn(
            "compact graph entry selector alias 'X' missing from aliases declaration",
            result.steps[0].errors,
        )

    def test_line_protocol_rejects_entry_alias_wrong_profile_kind(self) -> None:
        result = _run_entry_scenario(
            "python.compact-graph-entry-wrong-kind",
            (
                "print('[search-query] q=async selector=src/lib.rs alg=query-set')\n"
                "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                "print('aliases: graph:{G=search,O=owner,T=test}')\n"
                "print('O=owner:path(src/lib.rs)!owner;T=test:path(tests/lib.rs)!tests')\n"
                "print('G>{O:selects,T:covers}')\n"
                "print('rank=O,T frontier=O.owner,T.tests')\n"
                "print('entries=query-deps(O,T=>owners+imports)')"
            ),
        )

        self.assertEqual("fail", result.status)
        self.assertIn(
            "compact graph entry selector alias 'O' for profile 'query-deps' resolves to 'owner', expected 'query'",
            result.steps[0].errors,
        )

    def test_line_protocol_rejects_owner_tests_extra_test_selector(self) -> None:
        result = _run_entry_scenario(
            "python.compact-graph-owner-tests-extra-selector",
            (
                "print('[search-query] q=async selector=src/lib.rs alg=query-set')\n"
                "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                "print('aliases: graph:{G=search,O=owner,T=test}')\n"
                "print('O=owner:path(src/lib.rs)!owner;T=test:path(tests/lib.rs)!tests')\n"
                "print('G>{O:selects,T:covers}')\n"
                "print('rank=O,T frontier=O.owner,T.tests')\n"
                "print('entries=owner-tests(O,T=>covering-tests+fixtures)')"
            ),
        )

        self.assertEqual("fail", result.status)
        self.assertIn(
            "compact graph entry profile 'owner-tests' selector count 2 does not match schema contract owner",
            result.steps[0].errors,
        )


def _run_entry_scenario(scenario_id: str, python_script: str):
    with tempfile.TemporaryDirectory() as tmp:
        repo_root = Path(tmp)
        scenario_path = repo_root / "scenario.json"
        scenario_path.write_text(
            json.dumps(
                {
                    "id": scenario_id,
                    "language": "python",
                    "workdir": ".",
                    "steps": [
                        {
                            "id": "compact-graph-entries",
                            "command": ["python", "-c", python_script],
                            "expect": {"lineProtocol": True},
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )

        return run_scenario(repo_root, scenario_path)
