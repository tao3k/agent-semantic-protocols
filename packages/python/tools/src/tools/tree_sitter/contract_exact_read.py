"""Exact direct-source-read gates for tree-sitter rollout."""

from __future__ import annotations

from .contract_support import asp, contains, no_cache_noise, not_contains, pure_code


def check_exact_direct_read_contract(env: dict[str, str], asp_bin: str) -> None:
    _check_rust_exact_read(env, asp_bin)
    _check_typescript_exact_read(env, asp_bin)
    _check_python_exact_read(env, asp_bin)


def _check_rust_exact_read(env: dict[str, str], asp_bin: str) -> None:
    rust_read = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "languages/rust-lang-project-harness/src/cli/query.rs:31:46",
    )
    contains(rust_read, "[read-plan]", "rust exact read")
    contains(rust_read, "mode=range-frontier", "rust exact read")
    contains(rust_read, "frontier=S.code", "rust exact read")
    contains(rust_read, "omit=code", "rust exact read")
    contains(rust_read, "parse_query", "rust exact read")
    not_contains(rust_read, "pub(super) fn parse_query", "rust exact read")
    no_cache_noise(rust_read, "rust exact read")

    rust_code = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "languages/rust-lang-project-harness/src/cli/query.rs:31:46",
        "--workspace",
        ".",
        "--code",
    )
    pure_code(rust_code, "pub(super) fn parse_query", "rust exact code")


def _check_typescript_exact_read(env: dict[str, str], asp_bin: str) -> None:
    typescript_read = asp(
        env,
        asp_bin,
        "typescript",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "src/cli/protocol-tree-sitter-query.ts:55:58",
        "languages/typescript-lang-project-harness",
    )
    contains(typescript_read, "[read-owner]", "typescript exact read")
    contains(typescript_read, "window=1", "typescript exact read")
    contains(
        typescript_read,
        "|read path=src/cli/protocol-tree-sitter-query.ts",
        "typescript exact read",
    )
    contains(
        typescript_read,
        "read=src/cli/protocol-tree-sitter-query.ts",
        "typescript exact read",
    )
    contains(typescript_read, "next=direct-source-read", "typescript exact read")
    not_contains(typescript_read, "|code", "typescript exact read")
    not_contains(typescript_read, "text=", "typescript exact read")
    no_cache_noise(typescript_read, "typescript exact read")

    typescript_code = asp(
        env,
        asp_bin,
        "typescript",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "languages/typescript-lang-project-harness/src/cli/protocol-tree-sitter-query.ts:56:59",
        "--workspace",
        ".",
        "--code",
    )
    pure_code(
        typescript_code,
        "export function parseTreeSitterQueryArgs",
        "typescript exact code",
    )


def _check_python_exact_read(env: dict[str, str], asp_bin: str) -> None:
    python_read = asp(
        env,
        asp_bin,
        "python",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "src/python_lang_project_harness/_cli_query.py:20:60",
        "languages/python-lang-project-harness",
    )
    contains(python_read, "[read-owner]", "python exact read")
    contains(python_read, "window=1", "python exact read")
    contains(
        python_read,
        "|read path=src/python_lang_project_harness/_cli_query.py",
        "python exact read",
    )
    contains(
        python_read,
        "read=src/python_lang_project_harness/_cli_query.py",
        "python exact read",
    )
    contains(python_read, "next=direct-source-read", "python exact read")
    not_contains(python_read, "|code", "python exact read")
    not_contains(python_read, "text=", "python exact read")
    no_cache_noise(python_read, "python exact read")

    python_code = asp(
        env,
        asp_bin,
        "python",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "languages/python-lang-project-harness/src/python_lang_project_harness/_cli_query.py:20:60",
        "--workspace",
        ".",
        "--code",
    )
    pure_code(python_code, "def run_query_command", "python exact code")
