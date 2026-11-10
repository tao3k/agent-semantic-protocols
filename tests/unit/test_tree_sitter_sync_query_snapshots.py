# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Tests for tree-sitter query snapshot synchronization."""

from __future__ import annotations

import json
from pathlib import Path

from tools.tree_sitter.sync_query_snapshots import main


def test_sync_query_snapshots_excludes_highlights_and_writes_corpus_profile(
    tmp_path: Path,
) -> None:
    upstream = tmp_path / "tree-sitter-example"
    provider = tmp_path / "provider/tree-sitter-example"
    (upstream / "queries").mkdir(parents=True)
    (upstream / "test/corpus").mkdir(parents=True)
    (upstream / "queries/highlights.scm").write_text("(identifier) @variable\n")
    (upstream / "queries/locals.scm").write_text("(identifier) @local.reference\n")
    (upstream / "queries/injections.scm").write_text("(comment) @injection.content\n")
    (upstream / "test/corpus/definitions.txt").write_text(
        "================================================================================\n"
        "example\n"
        "================================================================================\n"
        "x\n"
        "---\n"
        "(source_file (identifier))\n",
    )
    (upstream / "tree-sitter.json").write_text(
        json.dumps(
            {
                "metadata": {
                    "version": "1.2.3",
                    "links": {"repository": "https://example.invalid/tree-sitter-example"},
                }
            }
        )
    )

    status = main(["--upstream", str(upstream), "--provider-dir", str(provider)])

    assert status == 0
    assert not (provider / "queries/highlights.scm").exists()
    assert (provider / "queries/locals.scm").read_text() == "(identifier) @local.reference\n"
    assert (
        provider / "queries/injections.scm"
    ).read_text() == "(comment) @injection.content\n"
    corpus_profile = json.loads((provider / "corpus-profile.json").read_text())
    assert corpus_profile["source"]["repository"] == "https://example.invalid/tree-sitter-example"
    assert corpus_profile["source"]["version"] == "1.2.3"
    assert corpus_profile["files"][0]["path"] == "test/corpus/definitions.txt"
    assert corpus_profile["files"][0]["caseCount"] == 1


def test_sync_query_snapshots_check_mode_reports_drift(tmp_path: Path) -> None:
    upstream = tmp_path / "tree-sitter-example"
    provider = tmp_path / "provider/tree-sitter-example"
    (upstream / "queries").mkdir(parents=True)
    (upstream / "test/corpus").mkdir(parents=True)
    (provider / "queries").mkdir(parents=True)
    (upstream / "queries/locals.scm").write_text("(identifier) @local.reference\n")
    (provider / "queries/locals.scm").write_text("(identifier) @stale.reference\n")
    (upstream / "test/corpus/definitions.txt").write_text("")
    (upstream / "tree-sitter.json").write_text(json.dumps({"metadata": {}}))

    status = main(
        [
            "--upstream",
            str(upstream),
            "--provider-dir",
            str(provider),
            "--check",
        ]
    )

    assert status == 1
    assert (provider / "queries/locals.scm").read_text() == "(identifier) @stale.reference\n"
