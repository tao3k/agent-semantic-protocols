# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Contract checks for the canonical ASP Server-first RFC."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RFC = ROOT / "docs" / "10-19-rfcs" / "10.05-interactive-graph-first-progressive-searchloop.org"
RFC_PARTS = [
    ROOT / "docs" / "10-19-rfcs" / name
    for name in [
        "10.05.00-searchloop-core-architecture.org",
        "10.05.10-public-search-playbook.org",
        "10.05.20-search-query-wire-replay.org",
        "10.05.30-runtime-invariants-ownership.org",
        "10.05.40-projection-materialization.org",
        "10.05.50-qualification-write-policy.org",
    ]
]
RFC_LEAN_SYSTEM = (
    ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.05-interactive-graph-first-progressive-searchloop"
    / "10.05.60-lean-typst-audit"
    / "00.13-rfc-lean-executable-mathematical-system.org"
)
REMOVED_STEM = "10.05-" + "cli-" + "first-harness-ux"
LEGACY_ROOT = ROOT / "docs" / "10-19-rfcs" / f"{REMOVED_STEM}.org"
LEGACY_TREE = ROOT / "docs" / "10-19-rfcs" / REMOVED_STEM


def test_server_first_rfc_is_the_only_10_05_architecture_root() -> None:
    text = RFC.read_text(encoding="utf-8")
    required = [
        "ASP Server-First Agent Search Architecture",
        "public gRPC ClientFrame",
        "thin typed CLI adapter",
        "Git history is the only historical",
    ]
    assert [term for term in required if term not in text] == []


def test_cli_first_rfc_tree_is_deleted() -> None:
    assert not LEGACY_ROOT.exists()
    assert not LEGACY_TREE.exists()


def test_searchloop_root_remains_a_bounded_module_index() -> None:
    lines = RFC.read_text(encoding="utf-8").splitlines()
    assert len(lines) <= 160
    assert all(path.exists() for path in RFC_PARTS)
    assert [path.name for path in RFC_PARTS if path.name not in "\n".join(lines)] == []


def test_split_owners_retain_the_normative_searchloop_surfaces() -> None:
    text = "\n".join(path.read_text(encoding="utf-8") for path in RFC_PARTS)
    required = [
        "* Global architecture invariant",
        "* Primary loop",
        "* Search/query wire and replay contract",
        "* Global invariants",
        "* Ownership map",
        "* Projection boundary",
        "* Qualification gates",
        "* Write policy",
    ]
    assert [term for term in required if term not in text] == []


def test_rfc_lean_system_binds_math_composition_rust_and_scenarios() -> None:
    text = RFC_LEAN_SYSTEM.read_text(encoding="utf-8")
    required = [
        "restore_reuse_iff_clean",
        "Content reuse is not generation reuse",
        "Recursive Scheme composition with bounded execution",
        "Search and exact Query are different algorithms",
        "resident_work_is_corpus_independent",
        "* Rust projection",
        "#+begin_src typst",
        "* Required Scenario evidence",
    ]
    assert [term for term in required if term not in text] == []
    assert (ROOT / "packages/proofs/ASPProof/SearchLoopRfcLeanSystem.lean").exists()
