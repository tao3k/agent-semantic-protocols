"""Validate Rust tree-sitter query snapshot sync tooling."""

import json
import subprocess
import sys
from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SYNC_SCRIPT = _REPO_ROOT / "tools" / "sync-tree-sitter-rust-queries.py"


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def test_sync_tree_sitter_rust_queries_updates_queries_and_corpus_profile(
    tmp_path: Path,
) -> None:
    upstream = tmp_path / "tree-sitter-rust"
    provider = tmp_path / "provider-tree-sitter-rust"

    _write_text(
        upstream / "tree-sitter.json",
        json.dumps(
            {
                "grammars": [
                    {
                        "highlights": ["queries/highlights.scm"],
                        "injections": ["queries/injections.scm"],
                        "tags": ["queries/tags.scm"],
                    }
                ],
                "metadata": {
                    "version": "0.test",
                },
            }
        ),
    )
    _write_text(upstream / "queries" / "highlights.scm", "(identifier) @variable\n")
    _write_text(
        upstream / "queries" / "injections.scm",
        "(token_tree) @injection.content\n",
    )
    _write_text(
        upstream / "queries" / "tags.scm",
        "(function_item name: (identifier) @name)\n",
    )
    _write_text(
        upstream / "test" / "corpus" / "declarations.txt",
        "\n".join(
            [
                "=" * 80,
                "Function declarations",
                "=" * 80,
                "",
                "fn main() {}",
                "",
                "-" * 80,
                "",
                "(source_file (function_item name: (identifier)))",
                "",
            ]
        ),
    )

    for name in ("calls", "cfg", "declarations", "imports", "macros"):
        _write_text(provider / "queries" / f"{name}.scm", f"(source_file) @{name}.root\n")

    result = subprocess.run(
        [
            sys.executable,
            str(_SYNC_SCRIPT),
            "--upstream",
            str(upstream),
            "--provider-dir",
            str(provider),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    assert "updated tree-sitter Rust query snapshots" in result.stdout
    assert not (provider / "queries" / "highlights.scm").exists()
    assert (provider / "queries" / "tags.scm").read_text() == (
        "(function_item name: (identifier) @name)\n"
    )

    corpus_profile = json.loads((provider / "corpus-profile.json").read_text())
    assert corpus_profile["source"]["version"] == "0.test"
    assert corpus_profile["files"] == [
        {
            "caseCount": 1,
            "lineCount": 9,
            "path": "test/corpus/declarations.txt",
            "sha256": corpus_profile["files"][0]["sha256"],
        }
    ]

    subprocess.run(
        [
            sys.executable,
            str(_SYNC_SCRIPT),
            "--upstream",
            str(upstream),
            "--provider-dir",
            str(provider),
            "--check",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
