from __future__ import annotations

from tools.tree_sitter.contract_support import (
    asp,
    contains,
    no_cache_noise,
    not_contains,
)


def check_search_read_plan_frontier_contract(
    env: dict[str, str], asp_bin: str
) -> None:
    _check_rust_search_frontier(env, asp_bin)
    _check_typescript_search_frontier(env, asp_bin)
    _check_python_search_frontier(env, asp_bin)
    _check_rust_read_plan(env, asp_bin)


def search_frontier(value: str, label: str) -> None:
    if "[graph-frontier]" not in value and "[search-fzf]" not in value:
        contains(value, "[graph-frontier]", label)
    if "profile=owner-query" not in value and "entries=owner-query" not in value:
        contains(value, "profile=owner-query", label)
    contains(value, "frontier=", label)
    contains(value, "owner:path(", label)
    if "item:symbol(" not in value and "query:term(" not in value:
        contains(value, "item:symbol(", label)
    no_cache_noise(value, label)


def search_pipe_frontier(value: str, label: str) -> None:
    contains(value, "[search-pipe]", label)
    contains(value, "queryQuality=", label)
    contains(value, "ownerCoverage=", label)
    contains(value, "nextCommand=", label)
    contains(value, "avoid=", label)
    no_cache_noise(value, label)


def _check_rust_search_frontier(env: dict[str, str], asp_bin: str) -> None:
    output = asp(
        env,
        asp_bin,
        "rust",
        "search",
        "pipe",
        "parse_query cli query command tests",
        "--workspace",
        "languages/rust-lang-project-harness",
        "--view",
        "seeds",
    )
    search_pipe_frontier(output, "rust search frontier")
    contains(output, "src/cli/query.rs", "rust search frontier")
    not_contains(output, "pub(super) fn parse_query", "rust search frontier")


def _check_typescript_search_frontier(env: dict[str, str], asp_bin: str) -> None:
    output = asp(
        env,
        asp_bin,
        "typescript",
        "search",
        "pipe",
        "parseTreeSitterQueryArgs owner tests",
        "--workspace",
        "languages/typescript-lang-project-harness",
        "--view",
        "seeds",
    )
    search_pipe_frontier(output, "typescript search frontier")
    contains(output, "src/cli/protocol.ts", "typescript search frontier")
    not_contains(output, "export function parseTreeSitterQueryArgs", "typescript search frontier")


def _check_python_search_frontier(env: dict[str, str], asp_bin: str) -> None:
    output = asp(
        env,
        asp_bin,
        "python",
        "search",
        "pipe",
        "run_query_command cli query command tests",
        "--workspace",
        "languages/python-lang-project-harness",
        "--view",
        "seeds",
    )
    search_pipe_frontier(output, "python search frontier")
    contains(output, "src/python_lang_project_harness/_cli_agent.py", "python search frontier")
    not_contains(output, "def run_query_command", "python search frontier")


def _check_rust_read_plan(env: dict[str, str], asp_bin: str) -> None:
    output = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "src/cli/query.rs",
        "--query",
        "parse_query",
        "--workspace",
        "languages/rust-lang-project-harness",
    )
    contains(output, "[search-owner]", "rust read-plan locator")
    contains(output, "structuralSelector=rust://", "rust read-plan locator")
    contains(output, "displayLineRange=", "rust read-plan locator")
    contains(output, "sourceLocatorHint=", "rust read-plan locator")
    contains(output, "codePolicy=requires-exact-code", "rust read-plan locator")
    contains(output, "next=query --code", "rust read-plan locator")
    contains(output, "parse_query", "rust read-plan locator")
    not_contains(output, "pub(super) fn parse_query", "rust read-plan locator")
    no_cache_noise(output, "rust read-plan locator")
