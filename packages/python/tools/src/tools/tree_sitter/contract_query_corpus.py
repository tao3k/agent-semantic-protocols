"""Query corpus gates for tree-sitter rollout."""

from __future__ import annotations

from .contract_support import run


QUERY_CORPUS_COMMANDS = (
    [
        "uv",
        "run",
        "--project",
        "packages/python",
        "--frozen",
        "python",
        "-m",
        "tools",
        "tree-sitter",
        "validate",
        "rust-query-corpus",
    ],
    [
        "uv",
        "run",
        "--project",
        "packages/python",
        "--frozen",
        "python",
        "-m",
        "tools",
        "tree-sitter",
        "validate",
        "typescript-query-corpus",
    ],
    [
        "uv",
        "run",
        "--project",
        "packages/python",
        "--frozen",
        "python",
        "-m",
        "tools",
        "tree-sitter",
        "validate",
        "python-query-corpus",
    ],
    [
        "uv",
        "run",
        "--project",
        "packages/python",
        "--frozen",
        "python",
        "-m",
        "tools",
        "tree-sitter",
        "validate",
        "json-abi-corpus",
    ],
)


def check_query_corpus_contracts(env: dict[str, str], _asp_bin: str) -> None:
    for command in QUERY_CORPUS_COMMANDS:
        run(command, env=env)
