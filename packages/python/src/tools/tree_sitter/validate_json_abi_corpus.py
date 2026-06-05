#!/usr/bin/env python3
"""Validate corpus cases against semantic tree-sitter JSON ABI shape."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from tools.console import emit

from .json_abi_corpus_cases import _load_cases, _validate_case


REPO_ROOT = Path(__file__).resolve().parents[5]
ASP_BIN = os.environ.get("SEMANTIC_AGENT_PROTOCOL_BIN", "asp")


@dataclass(frozen=True)
class CorpusConfig:
    language: str
    corpus_dir: Path
    extension: str

    @property
    def default_source_path(self) -> str:
        return str(Path("src") / f"corpus.{self.extension}")


CORPUS_CONFIGS = (
    CorpusConfig(
        language="rust",
        corpus_dir=REPO_ROOT
        / "languages/rust-lang-project-harness/tree-sitter/tree-sitter-rust/query-corpus",
        extension="rs",
    ),
    CorpusConfig(
        language="typescript",
        corpus_dir=REPO_ROOT
        / "languages/typescript-lang-project-harness/tree-sitter/tree-sitter-typescript/test/corpus",
        extension="ts",
    ),
    CorpusConfig(
        language="python",
        corpus_dir=REPO_ROOT
        / "languages/python-lang-project-harness/tree-sitter/tree-sitter-python/query-corpus",
        extension="py",
    ),
)

NATIVE_PROJECTION_OPTIONAL_CATALOGS = frozenset(
    {
        ("rust", "cfg"),
        ("rust", "macros"),
    }
)


def main() -> int:
    cases = [case for config in CORPUS_CONFIGS for case in _load_cases(config)]
    if not cases:
        raise AssertionError("no tree-sitter corpus cases discovered")
    for case in cases:
        _validate_case(
            case,
            asp_bin=ASP_BIN,
            repo_root=REPO_ROOT,
            optional_catalogs=NATIVE_PROJECTION_OPTIONAL_CATALOGS,
        )
    emit(f"tree-sitter JSON ABI corpus is valid: cases={len(cases)}")
    return 0




if __name__ == "__main__":
    sys.exit(main())
