"""Validate Rust ASP query corpus fixtures."""

import subprocess

def test_tree_sitter_rust_query_corpus_contract_is_valid() -> None:
    result = subprocess.run(
        [
            "uv",
            "run",
            "--project",
            "packages/python",
            "python",
            "-m",
            "tools",
            "tree-sitter",
            "validate",
            "rust-query-corpus",
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    assert "tree-sitter Rust query corpus is valid" in result.stdout


def test_tree_sitter_python_query_corpus_contract_is_valid() -> None:
    result = subprocess.run(
        [
            "uv",
            "run",
            "--project",
            "packages/python",
            "python",
            "-m",
            "tools",
            "tree-sitter",
            "validate",
            "python-query-corpus",
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    assert "tree-sitter Python query corpus is valid" in result.stdout


def test_tree_sitter_typescript_query_corpus_contract_is_valid() -> None:
    result = subprocess.run(
        [
            "uv",
            "run",
            "--project",
            "packages/python",
            "python",
            "-m",
            "tools",
            "tree-sitter",
            "validate",
            "typescript-query-corpus",
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    assert "tree-sitter TypeScript query corpus is valid" in result.stdout
