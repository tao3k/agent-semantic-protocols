#!/usr/bin/env python3
"""Refresh TypeScript tree-sitter query corpus metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

from tools.console import emit
from tools.paths import repo_root


SEPARATOR = "=" * 80
REPO_ROOT = repo_root()
GRAMMAR_ROOT = (
    REPO_ROOT
    / "languages"
    / "typescript-lang-project-harness"
    / "tree-sitter"
    / "tree-sitter-typescript"
)
GRAMMAR_PROFILE = GRAMMAR_ROOT / "grammar-profile.json"
CORPUS_PROFILE = GRAMMAR_ROOT / "corpus-profile.json"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    profile = build_profile()
    rendered = json.dumps(profile, indent=2) + "\n"
    if args.check:
        if CORPUS_PROFILE.read_text() != rendered:
            raise AssertionError(f"{CORPUS_PROFILE} is stale")
        emit("tree-sitter TypeScript query corpus profile is current")
        return 0
    CORPUS_PROFILE.write_text(rendered)
    emit(f"updated {CORPUS_PROFILE.relative_to(REPO_ROOT)}")
    return 0


def build_profile() -> dict[str, object]:
    grammar_profile = json.loads(GRAMMAR_PROFILE.read_text())
    corpus_root = GRAMMAR_ROOT / "test" / "corpus"
    return {
        "schemaVersion": "1",
        "corpusRoot": "test/corpus",
        "source": grammar_profile["upstream"],
        "files": [corpus_file_entry(path) for path in sorted(corpus_root.glob("*.txt"))],
    }


def corpus_file_entry(path: Path) -> dict[str, object]:
    text = path.read_text()
    return {
        "path": path.relative_to(GRAMMAR_ROOT).as_posix(),
        "caseCount": corpus_case_count(text.splitlines()),
        "lineCount": len(text.splitlines()),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def corpus_case_count(lines: list[str]) -> int:
    count = 0
    index = 0
    while index + 2 < len(lines):
        if lines[index] == SEPARATOR and lines[index + 2] == SEPARATOR:
            count += 1
            index += 3
            continue
        index += 1
    return count


if __name__ == "__main__":
    sys.exit(main())
