"""Frontier and pure-code stdout gates for tree-sitter query."""

from __future__ import annotations

from .contract_support import (
    asp,
    asp_expect_fail,
    contains,
    json_false,
    json_string,
    no_cache_noise,
    not_contains,
    pure_code,
)


def check_frontier_code_contract(env: dict[str, str], asp_bin: str) -> None:
    _check_rust_frontier_code(env, asp_bin)
    _check_typescript_frontier_code(env, asp_bin)
    _check_python_frontier_code(env, asp_bin)


def _check_rust_frontier_code(env: dict[str, str], asp_bin: str) -> None:
    rust_locate = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "--treesitter-query",
        "(function_item name: (identifier) @function.name)",
        "--selector",
        "src/cli/query.rs",
        "languages/rust-lang-project-harness",
    )
    contains(rust_locate, "[query-treesitter]", "rust locate")
    contains(rust_locate, "frontier=I.code", "rust locate")
    contains(rust_locate, "omit=code,full-node-list,capture-text", "rust locate")
    contains(rust_locate, "ts=identifier/name", "rust locate")
    contains(rust_locate, "parse_query", "rust locate")
    not_contains(rust_locate, "pub(super) fn parse_query", "rust locate")
    no_cache_noise(rust_locate, "rust locate")

    rust_code = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "--treesitter-query",
        '(function_item name: (identifier) @function.name (#eq? @function.name "parse_query"))',
        "--selector",
        "languages/rust-lang-project-harness/src/cli/query.rs:31:46",
        "--workspace",
        ".",
        "--code",
    )
    pure_code(rust_code, "pub(super) fn parse_query", "rust code")

    rust_no_selector = asp_expect_fail(
        env,
        asp_bin,
        "rust",
        "query",
        "--treesitter-query",
        '(function_item name: (identifier) @function.name (#eq? @function.name "parse_query"))',
        "--code",
    )
    contains(
        rust_no_selector,
        "tree-sitter query --code requires an exact --selector",
        "rust no selector",
    )

    rust_json = asp(
        env,
        asp_bin,
        "rust",
        "query",
        "--treesitter-query",
        '(function_item name: (identifier) @function.name (#eq? @function.name "parse_query"))',
        "--selector",
        "src/cli/query.rs",
        "--json",
        "languages/rust-lang-project-harness",
    )
    json_string(
        rust_json,
        "schemaId",
        "agent.semantic-protocols.semantic-tree-sitter-query",
        "rust json",
    )
    json_string(rust_json, "adapterMode", "native-projection", "rust json")
    json_string(rust_json, "compatibilityLevel", "native-only", "rust json")
    contains(rust_json, '"nativeFactRefs": [', "rust json")
    contains(rust_json, "rust:item:src/cli/query.rs:", "rust json")
    contains(rust_json, ":parse_query", "rust json")
    json_false(rust_json, "rawSourceStored", "rust json")


def _check_typescript_frontier_code(env: dict[str, str], asp_bin: str) -> None:
    ts_locate = asp(
        env,
        asp_bin,
        "typescript",
        "query",
        "--treesitter-query",
        "(function_declaration name: (identifier) @function.name)",
        "--selector",
        "src/cli/protocol-tree-sitter-query.ts",
        "languages/typescript-lang-project-harness",
    )
    contains(ts_locate, "src/cli/protocol-tree-sitter-query.ts:", "typescript locate")
    contains(ts_locate, "parseTreeSitterQueryArgs", "typescript locate")
    not_contains(ts_locate, "export function parseTreeSitterQueryArgs", "typescript locate")
    no_cache_noise(ts_locate, "typescript locate")

    ts_code = asp(
        env,
        asp_bin,
        "typescript",
        "query",
        "--treesitter-query",
        '(function_declaration name: (identifier) @function.name (#eq? @function.name "parseTreeSitterQueryArgs"))',
        "--selector",
        "languages/typescript-lang-project-harness/src/cli/protocol-tree-sitter-query.ts:56:61",
        "--workspace",
        ".",
        "--code",
    )
    pure_code(ts_code, "export function parseTreeSitterQueryArgs", "typescript code")

    ts_json = asp(
        env,
        asp_bin,
        "typescript",
        "query",
        "--treesitter-query",
        '(function_declaration name: (identifier) @function.name (#eq? @function.name "parseTreeSitterQueryArgs"))',
        "--selector",
        "src/cli/protocol-tree-sitter-query.ts",
        "--json",
        "languages/typescript-lang-project-harness",
    )
    json_string(
        ts_json,
        "schemaId",
        "agent.semantic-protocols.semantic-tree-sitter-query",
        "typescript json",
    )
    json_string(ts_json, "adapterMode", "native-projection", "typescript json")
    json_string(ts_json, "compatibilityLevel", "native-only", "typescript json")
    contains(ts_json, '"nativeFactRefs"', "typescript json")
    contains(ts_json, "typescript:item:src/cli/protocol-tree-sitter-query.ts:", "typescript json")
    contains(ts_json, ":parseTreeSitterQueryArgs", "typescript json")
    json_false(ts_json, "rawSourceStored", "typescript json")
    json_string(ts_json, "nodeType", "identifier", "typescript json")
    json_string(ts_json, "field", "name", "typescript json")
    json_string(ts_json, "nativeNodeType", "function_declaration", "typescript json")


def _check_python_frontier_code(env: dict[str, str], asp_bin: str) -> None:
    python_locate = asp(
        env,
        asp_bin,
        "python",
        "query",
        "--treesitter-query",
        "(function_definition name: (identifier) @function.name)",
        "--selector",
        "src/python_lang_project_harness/_cli_query.py",
        "languages/python-lang-project-harness",
    )
    contains(python_locate, "src/python_lang_project_harness/_cli_query.py:", "python locate")
    contains(python_locate, "run_query_command", "python locate")
    not_contains(python_locate, "def run_query_command", "python locate")
    no_cache_noise(python_locate, "python locate")

    python_code = asp(
        env,
        asp_bin,
        "python",
        "query",
        "--treesitter-query",
        '(function_definition name: (identifier) @function.name (#eq? @function.name "run_query_command"))',
        "--selector",
        "src/python_lang_project_harness/_cli_query.py",
        "--workspace",
        "languages/python-lang-project-harness",
        "--code",
    )
    pure_code(python_code, "def run_query_command", "python code")

    python_json = asp(
        env,
        asp_bin,
        "python",
        "query",
        "--treesitter-query",
        '(function_definition name: (identifier) @function.name (#eq? @function.name "run_query_command"))',
        "--selector",
        "src/python_lang_project_harness/_cli_query.py",
        "--json",
        "languages/python-lang-project-harness",
    )
    json_string(
        python_json,
        "schemaId",
        "agent.semantic-protocols.semantic-tree-sitter-query",
        "python json",
    )
    json_string(python_json, "adapterMode", "native-projection", "python json")
    json_string(python_json, "compatibilityLevel", "native-only", "python json")
    contains(python_json, '"nativeFactRefs"', "python json")
    contains(
        python_json,
        "python:ast:src/python_lang_project_harness/_cli_query.py:20:60:run_query_command",
        "python json",
    )
    json_false(python_json, "rawSourceStored", "python json")
    json_string(python_json, "nodeType", "identifier", "python json")
    json_string(python_json, "field", "name", "python json")
    json_string(python_json, "nativeNodeType", "function_definition", "python json")
