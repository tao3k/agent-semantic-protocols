"""Search and read-plan frontier gates for tree-sitter rollout."""

from __future__ import annotations

from .contract_support import asp, contains, no_cache_noise, not_contains, search_frontier


def check_search_read_plan_frontier_contract(
    env: dict[str, str], asp_bin: str
) -> None:
    _check_rust_search_frontier(env, asp_bin)
    _check_typescript_search_frontier(env, asp_bin)
    _check_python_search_frontier(env, asp_bin)
    _check_rust_read_plan(env, asp_bin)


def _check_rust_search_frontier(env: dict[str, str], asp_bin: str) -> None:
    output = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "**/*.rs",
        "--term",
        "parse_query",
        "--surface",
        "owners,tests",
        "--view",
        "seeds",
        "languages/rust-lang-project-harness",
    )
    search_frontier(output, "rust search frontier")
    contains(output, "O=owner:path(", "rust search frontier")
    not_contains(output, "pub(super) fn parse_query", "rust search frontier")


def _check_typescript_search_frontier(env: dict[str, str], asp_bin: str) -> None:
    output = asp(
        env,
        asp_bin,
        "typescript",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "**/*.ts",
        "--term",
        "parseTreeSitterQueryArgs",
        "--surface",
        "owners,tests",
        "--view",
        "seeds",
        "languages/typescript-lang-project-harness",
    )
    search_frontier(output, "typescript search frontier")
    contains(output, "src/cli/protocol-tree-sitter-query.ts", "typescript search frontier")
    not_contains(output, "export function parseTreeSitterQueryArgs", "typescript search frontier")


def _check_python_search_frontier(env: dict[str, str], asp_bin: str) -> None:
    output = asp(
        env,
        asp_bin,
        "python",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "**/*.py",
        "--term",
        "run_query_command",
        "--surface",
        "owners,tests",
        "--view",
        "seeds",
        "languages/python-lang-project-harness",
    )
    search_frontier(output, "python search frontier")
    contains(output, "src/python_lang_project_harness/_cli_query.py", "python search frontier")
    not_contains(output, "def run_query_command", "python search frontier")


def _check_rust_read_plan(env: dict[str, str], asp_bin: str) -> None:
    output = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "src/cli/query.rs:1:260",
        "languages/rust-lang-project-harness",
    )
    contains(output, "[read-plan]", "rust read-plan")
    contains(output, "mode=range-frontier", "rust read-plan")
    contains(output, "frontier=S.code", "rust read-plan")
    contains(output, "omit=code", "rust read-plan")
    contains(output, "avoid=repeat-wide-read", "rust read-plan")
    contains(output, "parse_query", "rust read-plan")
    not_contains(output, "pub(super) fn parse_query", "rust read-plan")
    no_cache_noise(output, "rust read-plan")
