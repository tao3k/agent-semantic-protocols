"""Validate Python tree-sitter-compatible query corpus fixtures."""

from __future__ import annotations

import json
from pathlib import Path

from tools.console import emit

from .contract import assert_asp_tree_sitter_contract

REPO_ROOT = Path(__file__).resolve().parents[5]
GRAMMAR_ROOT = (
    REPO_ROOT
    / "languages"
    / "python-lang-project-harness"
    / "tree-sitter"
    / "tree-sitter-python"
)
GRAMMAR_PROFILE = GRAMMAR_ROOT / "grammar-profile.json"
VALIDATOR = "asp-tree-sitter-validate-python-query-corpus"


def main() -> int:
    profile = json.loads(GRAMMAR_PROFILE.read_text())
    assert_asp_tree_sitter_contract(
        profile,
        extra_paths=("packages/python/src/tools/tree_sitter/validate_python_query_corpus.py",),
    )
    if profile["aspWorkspace"]["queryCorpusValidator"] != VALIDATOR:
        raise AssertionError("grammar-profile.json: aspWorkspace.queryCorpusValidator is stale")
    if profile["queryCorpus"]["validator"] != VALIDATOR:
        raise AssertionError("grammar-profile.json: queryCorpus.validator is stale")
    _validate_catalog_captures(profile)
    case_count = _validate_corpus_cases(profile)
    if case_count == 0:
        raise AssertionError("Python query corpus has no capture cases")
    emit("tree-sitter Python query corpus is valid")
    return 0


def _validate_catalog_captures(profile: dict[str, object]) -> None:
    for catalog in profile["catalogs"]:
        catalog_id = catalog["id"]
        query_file = GRAMMAR_ROOT / "queries" / f"{catalog_id}.scm"
        query_text = query_file.read_text()
        for capture in catalog["captures"]:
            if f"@{capture}" not in query_text:
                raise AssertionError(f"{query_file}: missing capture @{capture}")


def _validate_corpus_cases(profile: dict[str, object]) -> int:
    captures_by_catalog = {
        catalog["id"]: set(catalog["captures"])
        for catalog in profile["catalogs"]
    }
    case_count = 0
    for corpus_file in sorted((GRAMMAR_ROOT / "query-corpus").glob("*.txt")):
        current_catalog = ""
        for line in corpus_file.read_text().splitlines():
            if line.startswith("catalog: "):
                current_catalog = line.removeprefix("catalog: ")
            elif line.startswith("capture "):
                if not current_catalog:
                    raise AssertionError(f"{corpus_file}: capture case missing catalog")
                capture = line.removeprefix("capture ").split(" node=", maxsplit=1)[0]
                if capture not in captures_by_catalog.get(current_catalog, set()):
                    raise AssertionError(
                        f"{corpus_file}: unknown capture {capture} for {current_catalog}"
                    )
                case_count += 1
    return case_count


if __name__ == "__main__":
    raise SystemExit(main())
