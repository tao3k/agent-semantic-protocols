"""Validate Rust ASP query corpus fixtures."""

import subprocess
import sys
from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[2]
_VALIDATOR = _REPO_ROOT / "tools" / "validate-tree-sitter-rust-query-corpus.py"
_PYTHON_VALIDATOR = _REPO_ROOT / "tools" / "validate-tree-sitter-python-query-corpus.sh"
_TYPESCRIPT_VALIDATOR = _REPO_ROOT / "tools" / "validate-tree-sitter-typescript-query-corpus.py"

def test_tree_sitter_rust_query_corpus_contract_is_valid() -> None:
    result = subprocess.run(
        [sys.executable, str(_VALIDATOR)],
        check=True,
        text=True,
        capture_output=True,
    )

    assert "tree-sitter Rust query corpus is valid" in result.stdout


def test_tree_sitter_python_query_corpus_contract_is_valid() -> None:
    result = subprocess.run(
        ["bash", str(_PYTHON_VALIDATOR)],
        check=True,
        text=True,
        capture_output=True,
    )

    assert "tree-sitter Python query corpus is valid" in result.stdout


def test_tree_sitter_typescript_query_corpus_contract_is_valid() -> None:
    result = subprocess.run(
        [sys.executable, str(_TYPESCRIPT_VALIDATOR)],
        check=True,
        text=True,
        capture_output=True,
    )

    assert "tree-sitter TypeScript query corpus is valid" in result.stdout
