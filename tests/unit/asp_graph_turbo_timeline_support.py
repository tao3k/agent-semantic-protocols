"""Shared artifact fixtures for graph turbo timeline tests."""

from __future__ import annotations

import json
import os
from pathlib import Path


def write_microburst_repeat_artifacts(root: Path) -> None:
    search_dir = root / "search"
    query_dir = root / "query"
    prompt_dir = root / "prompt-output"
    for directory in (search_dir, query_dir, prompt_dir):
        directory.mkdir()

    write_timeline_json(
        search_dir / "python-search-fzf-a.json",
        _search_packet("python", "search/fzf", query="semantic type"),
        mtime=1000,
    )
    write_timeline_json(
        prompt_dir / "mixed.command.json",
        {
            "schemaId": "agent.semantic-protocols.client-prompt-output-command",
            "providerCommands": _mixed_provider_commands(),
        },
        mtime=1005,
    )
    write_timeline_json(
        query_dir / "python-query-owner-items-a.json",
        {
            "schemaId": "agent.semantic-protocols.semantic-query-packet",
            "languageId": "python",
            "method": "query/owner-items",
            "ownerPath": "src/" + "types.py",
        },
        mtime=1021,
    )
    write_timeline_json(
        search_dir / "python-search-fzf-b.json",
        _search_packet("python", "search/fzf", query="semantic type"),
        mtime=1026,
    )
    for name, mtime in (("a", 1036), ("b", 1040)):
        write_timeline_json(
            search_dir / f"rust-search-owner-{name}.json",
            _search_packet(
                "rust",
                "search/owner",
                owner="crates/agent-semantic-protocol/src/command/provider.rs",
            ),
            mtime=mtime,
        )


def _mixed_provider_commands() -> list[dict[str, object]]:
    return [
        {
            "argv": ["py-harness", "search", "owner", "src/" + "types.py"],
            "languageId": "python",
        },
        {
            "argv": ["rs-harness", "search", "owner", "src/" + "lib.rs"],
            "languageId": "rust",
        },
        {
            "argv": [
                "rs-harness",
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "src/" + "lib.rs",
                ".",
            ],
            "languageId": "rust",
        },
        {
            "argv": [
                "rs-harness",
                "query",
                "--selector",
                "src/" + "lib.rs",
                "--code",
                ".",
            ],
            "languageId": "rust",
        },
        {
            "argv": [
                "rs-harness",
                "search",
                "--view",
                "seeds",
                "owner",
                "src/" + "lib.rs",
            ],
            "languageId": "rust",
        },
    ]


def _search_packet(
    language: str, method: str, *, query: str = "", owner: str = ""
) -> dict[str, object]:
    packet = {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "languageId": language,
        "method": method,
    }
    if query:
        packet["query"] = query
    if owner:
        packet["ownerPath"] = owner
    return packet


def write_timeline_json(path: Path, value: dict[str, object], *, mtime: int) -> None:
    path.write_text(json.dumps(value), encoding="utf-8")
    os.utime(path, (mtime, mtime))


def write_timeline_prime(path: Path, *, mtime: int) -> None:
    write_timeline_json(
        path,
        {
            "schemaId": "agent.semantic-protocols.semantic-search-packet",
            "languageId": "rust",
            "method": "search/prime",
            "owners": [{"path": "src/lib.rs"}],
        },
        mtime=mtime,
    )
