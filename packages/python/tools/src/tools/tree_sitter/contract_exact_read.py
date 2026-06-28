from __future__ import annotations

from tools.tree_sitter.contract_support import (
    asp,
    contains,
    no_cache_noise,
    not_contains,
)

DEPRECATED_DIRECT_READ_NEXT = "next=" + "-".join(("direct", "source", "read"))


def check_exact_direct_read_contract(env: dict[str, str], asp_bin: str) -> None:
    _check_rust_exact_read(env, asp_bin)
    _check_typescript_exact_read(env, asp_bin)
    _check_python_exact_read(env, asp_bin)


def locator(value: str, *, language: str, symbol: str, label: str) -> None:
    contains(value, "[search-owner]", label)
    contains(value, f"structuralSelector={language}://", label)
    contains(value, "displayLineRange=", label)
    contains(value, "sourceLocatorHint=", label)
    contains(value, "codePolicy=requires-exact-code", label)
    contains(value, "next=query --code", label)
    contains(value, symbol, label)
    not_contains(value, DEPRECATED_DIRECT_READ_NEXT, label)
    not_contains(value, "|code", label)
    not_contains(value, "text=", label)
    no_cache_noise(value, label)


def pure_code(value: str, signature: str, label: str) -> None:
    contains(value, signature, label)
    for needle in (
        "[query-treesitter]",
        "[read-owner]",
        "[read-plan]",
        "[search-owner]",
        "|code",
        "text=",
        "frontier=",
        "displayLineRange=",
        "sourceLocatorHint=",
    ):
        not_contains(value, needle, label)
    no_cache_noise(value, label)


def _check_rust_exact_read(env: dict[str, str], asp_bin: str) -> None:
    rust_locator = asp(
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
    locator(
        rust_locator,
        language="rust",
        symbol="parse_query",
        label="rust exact locator",
    )
    not_contains(rust_locator, "pub(super) fn parse_query", "rust exact locator")

    rust_code = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "src/cli/query.rs",
        "--query",
        "parse_query",
        "--workspace",
        "languages/rust-lang-project-harness",
        "--code",
    )
    pure_code(rust_code, "pub(super) fn parse_query", "rust exact code")


def _check_typescript_exact_read(env: dict[str, str], asp_bin: str) -> None:
    typescript_locator = asp(
        env,
        asp_bin,
        "typescript",
        "query",
        "src/cli/protocol-tree-sitter-query.ts",
        "--query",
        "parseTreeSitterQueryArgs",
        "--workspace",
        "languages/typescript-lang-project-harness",
    )
    locator(
        typescript_locator,
        language="typescript",
        symbol="parseTreeSitterQueryArgs",
        label="typescript exact locator",
    )
    not_contains(
        typescript_locator,
        "export function parseTreeSitterQueryArgs",
        "typescript exact locator",
    )

    typescript_code = asp(
        env,
        asp_bin,
        "typescript",
        "query",
        "src/cli/protocol-tree-sitter-query.ts",
        "--query",
        "parseTreeSitterQueryArgs",
        "--workspace",
        "languages/typescript-lang-project-harness",
        "--code",
    )
    pure_code(
        typescript_code,
        "export function parseTreeSitterQueryArgs",
        "typescript exact code",
    )


def _check_python_exact_read(env: dict[str, str], asp_bin: str) -> None:
    python_locator = asp(
        env,
        asp_bin,
        "python",
        "query",
        "src/python_lang_project_harness/_cli_query.py",
        "--term",
        "run_query_command",
        "--workspace",
        "languages/python-lang-project-harness",
    )
    locator(
        python_locator,
        language="python",
        symbol="run_query_command",
        label="python exact locator",
    )
    not_contains(python_locator, "def run_query_command", "python exact locator")

    python_code = asp(
        env,
        asp_bin,
        "python",
        "query",
        "src/python_lang_project_harness/_cli_query.py",
        "--term",
        "run_query_command",
        "--workspace",
        "languages/python-lang-project-harness",
        "--code",
    )
    pure_code(python_code, "def run_query_command", "python exact code")
