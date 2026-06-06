#!/usr/bin/env python3
"""Validate Rust ASP tree-sitter query corpus capture contracts."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

from tools.console import emit
from tools.paths import repo_root

from .contract import assert_asp_tree_sitter_contract
from .rust_query_corpus_cases import _parse_cases, _validate_case


_REPO_ROOT = repo_root()
_PROVIDER_DIR = (
    _REPO_ROOT / "languages/rust-lang-project-harness/tree-sitter/tree-sitter-rust"
)
_GIT_REV_RE = re.compile(r"^[0-9a-f]{40}$")


def _catalogs(profile: dict) -> dict[str, dict]:
    return {catalog["id"]: catalog for catalog in profile.get("catalogs", [])}


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def _current_asp_revision() -> str:
    return subprocess.check_output(
        ["git", "-C", str(_REPO_ROOT), "rev-parse", "HEAD"],
        text=True,
        stderr=subprocess.DEVNULL,
    ).strip()


def _validate_asp_workspace_profile(
    profile: dict,
    check_current_revision: bool,
) -> list[str]:
    workspace = profile.get("aspWorkspace")
    if not isinstance(workspace, dict):
        return ["grammar-profile.json: missing aspWorkspace provenance"]

    errors = []
    if workspace.get("owner") != "main-asp":
        errors.append("grammar-profile.json: aspWorkspace.owner must be main-asp")
    if not workspace.get("repository"):
        errors.append("grammar-profile.json: aspWorkspace.repository is required")
    revision = workspace.get("revision")
    if not isinstance(revision, str) or not _GIT_REV_RE.match(revision):
        errors.append("grammar-profile.json: aspWorkspace.revision must be a 40-hex git rev")
    if (
        workspace.get("queryCorpusValidator")
        != "asp-tree-sitter-validate-rust-query-corpus"
    ):
        errors.append("grammar-profile.json: aspWorkspace.queryCorpusValidator is stale")
    if check_current_revision and isinstance(revision, str):
        current_revision = _current_asp_revision()
        if revision != current_revision:
            errors.append(
                "grammar-profile.json: aspWorkspace.revision does not match current ASP "
                f"HEAD {current_revision}"
            )
    return errors


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--provider-dir",
        default=_PROVIDER_DIR,
        type=Path,
        help="Provider tree-sitter-rust catalog directory.",
    )
    parser.add_argument(
        "--check-current-asp-revision",
        action="store_true",
        help="Also require aspWorkspace.revision to match the current ASP checkout HEAD.",
    )
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    provider_dir = args.provider_dir.resolve()
    errors = _query_corpus_contract_errors(
        provider_dir,
        check_current_revision=args.check_current_asp_revision,
    )
    if errors:
        for error in errors:
            emit(error, file=sys.stderr)
        return 1
    emit("tree-sitter Rust query corpus is valid")
    return 0


def _query_corpus_contract_errors(
    provider_dir: Path,
    *,
    check_current_revision: bool,
) -> list[str]:
    profile = _load_json(provider_dir / "grammar-profile.json")
    assert_asp_tree_sitter_contract(
        profile,
        extra_paths=(
            "packages/python/tools/src/tools/tree_sitter/validate_rust_query_corpus.py",
            "packages/python/tools/src/tools/tree_sitter/rust_query_corpus_cases.py",
        ),
    )
    corpus_profile = _load_json(provider_dir / "corpus-profile.json")
    errors = _validate_asp_workspace_profile(
        profile,
        check_current_revision,
    )
    errors.extend(_query_corpus_profile_errors(profile))
    errors.extend(_validate_query_corpus(provider_dir, profile, corpus_profile))
    return errors


def _query_corpus_profile_errors(profile: dict) -> list[str]:
    errors: list[str] = []
    query_corpus = profile.get("queryCorpus", {})
    if query_corpus.get("path") != "tree-sitter/tree-sitter-rust/query-corpus":
        errors.append("grammar-profile.json: queryCorpus.path is stale")
    if (
        query_corpus.get("validator")
        != "asp-tree-sitter-validate-rust-query-corpus"
    ):
        errors.append("grammar-profile.json: queryCorpus.validator is stale")
    return errors


def _validate_query_corpus(
    provider_dir: Path,
    profile: dict,
    corpus_profile: dict,
) -> list[str]:
    errors: list[str] = []
    query_corpus = profile.get("queryCorpus", {})
    allowed_fixture_cases = set(query_corpus.get("realLibraryCases", []))
    upstream_corpus_paths = {
        item["path"] for item in corpus_profile.get("files", [])
    }
    catalogs = _catalogs(profile)
    corpus_dir = provider_dir / "query-corpus"
    for path in sorted(corpus_dir.glob("*.txt")):
        for case in _parse_cases(path):
            errors.extend(
                _validate_case(
                    case,
                    provider_dir,
                    catalogs,
                    upstream_corpus_paths,
                    allowed_fixture_cases,
                )
            )
    return errors


if __name__ == "__main__":
    raise SystemExit(main())
