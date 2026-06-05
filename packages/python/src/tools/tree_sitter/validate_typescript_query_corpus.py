#!/usr/bin/env python3
"""Validate TypeScript tree-sitter-compatible query corpus fixtures."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from tools.console import emit

from .contract import assert_asp_tree_sitter_contract


SEPARATOR = "=" * 80
EXPECTED_SEPARATOR = "-" * 80
REPO_ROOT = Path(__file__).resolve().parents[5]
PROVIDER_ROOT = REPO_ROOT / "languages" / "typescript-lang-project-harness"
GRAMMAR_ROOT = PROVIDER_ROOT / "tree-sitter" / "tree-sitter-typescript"
CORPUS_ROOT = GRAMMAR_ROOT / "test" / "corpus"
PROFILE_PATH = GRAMMAR_ROOT / "grammar-profile.json"
CLI_PATH = PROVIDER_ROOT / "dist" / "src" / "cli" / "main.js"
SYNC_SCRIPT = (
    REPO_ROOT
    / "packages"
    / "python"
    / "src"
    / "tools"
    / "tree_sitter"
    / "sync_typescript_query_corpus.py"
)


@dataclass(frozen=True)
class CorpusCase:
    title: str
    catalog: str
    file_path: str
    source: str
    expected: str


def main() -> int:
    profile = json.loads(PROFILE_PATH.read_text())
    validate_profile(profile)
    validate_corpus_profile()
    validate_catalog_files(profile)
    build_provider_cli()
    cases = [case for path in sorted(CORPUS_ROOT.glob("*.txt")) for case in parse_corpus(path)]
    if not cases:
        raise AssertionError(f"no TypeScript query corpus cases found in {CORPUS_ROOT}")
    for case in cases:
        validate_case(case)
    emit(f"tree-sitter TypeScript query corpus is valid: cases={len(cases)}")
    return 0


def validate_profile(profile: dict[str, object]) -> None:
    assert profile["grammarId"] == "tree-sitter-typescript"
    assert profile["grammarProfileVersion"] == "2026-06-05.v1"
    assert profile["sourceAuthority"] == "native-parser-adapter"
    assert_asp_tree_sitter_contract(
        profile,
        extra_paths=("packages/python/src/tools/tree_sitter/validate_typescript_query_corpus.py",),
    )
    assert (
        profile["aspWorkspace"]["queryCorpusValidator"]
        == "asp-tree-sitter-validate-typescript-query-corpus"
    )
    assert profile["queryCorpus"]["path"] == "tree-sitter/tree-sitter-typescript/test/corpus"
    assert (
        profile["queryCorpus"]["validator"]
        == "asp-tree-sitter-validate-typescript-query-corpus"
    )
    assert profile["corpusProfilePath"] == "tree-sitter/tree-sitter-typescript/corpus-profile.json"


def validate_corpus_profile() -> None:
    subprocess.run(
        [sys.executable, str(SYNC_SCRIPT), "--check"],
        check=True,
        text=True,
        capture_output=True,
    )


def validate_catalog_files(profile: dict[str, object]) -> None:
    for catalog in profile["catalogs"]:
        source = (PROVIDER_ROOT / catalog["path"]).read_text()
        for capture in catalog["captures"]:
            assert f"@{capture}" in source, f"{capture} missing from {catalog['path']}"


def build_provider_cli() -> None:
    subprocess.run(
        ["npm", "--prefix", str(PROVIDER_ROOT), "run", "build"],
        check=True,
        text=True,
        stdout=subprocess.DEVNULL,
    )


def parse_corpus(path: Path) -> list[CorpusCase]:
    lines = path.read_text().splitlines()
    cases: list[CorpusCase] = []
    index = 0
    while index < len(lines):
        while index < len(lines) and lines[index] == "":
            index += 1
        if index >= len(lines):
            break
        assert lines[index] == SEPARATOR, f"{path}:{index + 1}: expected case separator"
        title = lines[index + 1]
        assert lines[index + 2] == SEPARATOR, f"{path}:{index + 3}: expected title separator"
        index += 3
        metadata: dict[str, str] = {}
        while index < len(lines) and lines[index] != "":
            key, value = lines[index].split(":", 1)
            metadata[key.strip()] = value.strip()
            index += 1
        assert metadata.get("catalog"), f"{path}:{title}: missing catalog"
        assert metadata.get("file"), f"{path}:{title}: missing file"
        index += 1
        source: list[str] = []
        while index < len(lines) and lines[index] != EXPECTED_SEPARATOR:
            source.append(lines[index])
            index += 1
        assert index < len(lines), f"{path}:{title}: missing expected separator"
        index += 1
        expected: list[str] = []
        while index < len(lines) and lines[index] != SEPARATOR:
            expected.append(lines[index])
            index += 1
        cases.append(corpus_case(title, metadata, source, expected))
    return cases


def corpus_case(
    title: str,
    metadata: dict[str, str],
    source: list[str],
    expected: list[str],
) -> CorpusCase:
    return CorpusCase(
        title=title,
        catalog=metadata["catalog"],
        file_path=metadata["file"],
        source="\n".join(source) + "\n",
        expected="\n".join(trim_trailing_empty_lines(expected)),
    )


def validate_case(case: CorpusCase) -> None:
    with tempfile.TemporaryDirectory(prefix="ts-query-corpus-") as temp_dir:
        root = Path(temp_dir)
        source_path = root / case.file_path
        source_path.parent.mkdir(parents=True, exist_ok=True)
        source_path.write_text(case.source)
        result = subprocess.run(
            ["node", str(CLI_PATH), "query", "--catalog", case.catalog, str(root)],
            check=True,
            text=True,
            capture_output=True,
        )
        actual = result.stdout.strip()
        assert actual == case.expected, (
            f"{case.title} expected query output did not match\n"
            f"expected:\n{case.expected}\n\nactual:\n{actual}"
        )


def trim_trailing_empty_lines(lines: list[str]) -> list[str]:
    trimmed = list(lines)
    while trimmed and trimmed[-1] == "":
        trimmed.pop()
    return trimmed


def git_head() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    return result.stdout.strip()


if __name__ == "__main__":
    sys.exit(main())
