#!/usr/bin/env python3
"""Sync Rust tree-sitter query snapshots from an upstream checkout.

This is a development/CI maintenance tool. Runtime providers still embed the
committed query files into the binary with include_str!.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

from tools.console import emit
from tools.paths import repo_root


REPO_ROOT = repo_root()
DEFAULT_PROVIDER_DIR = (
    REPO_ROOT / "languages/rust-lang-project-harness/tree-sitter/tree-sitter-rust"
)
UPSTREAM_REFERENCE_QUERY_FILES = ("injections.scm", "tags.scm")
ASP_CATALOG_FILES = (
    "calls.scm",
    "cfg.scm",
    "declarations.scm",
    "imports.scm",
    "macros.scm",
)
CORPUS_PROFILE = "corpus-profile.json"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def upstream_revision(upstream: Path) -> str | None:
    try:
        return subprocess.check_output(
            ["git", "-C", str(upstream), "rev-parse", "HEAD"],
            stderr=subprocess.DEVNULL,
            text=True,
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def load_tree_sitter_json(upstream: Path) -> dict:
    path = upstream / "tree-sitter.json"
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as error:
        raise SystemExit(f"missing upstream tree-sitter.json: {path}") from error


def corpus_case_count(text: str) -> int:
    delimiter = "=" * 80
    return sum(1 for line in text.splitlines() if line == delimiter) // 2


def build_corpus_profile(upstream: Path, revision: str | None) -> bytes:
    corpus_root = upstream / "test/corpus"
    if not corpus_root.is_dir():
        raise SystemExit(f"missing upstream corpus dir: {corpus_root}")

    tree_sitter_json = load_tree_sitter_json(upstream)
    metadata = tree_sitter_json.get("metadata", {})
    files = []
    for path in sorted(corpus_root.glob("*.txt")):
        data = path.read_bytes()
        text = data.decode("utf-8")
        files.append(
            {
                "path": f"test/corpus/{path.name}",
                "sha256": sha256_bytes(data),
                "lineCount": len(text.splitlines()),
                "caseCount": corpus_case_count(text),
            }
        )

    profile = {
        "schemaVersion": "1",
        "source": {
            "repository": "https://github.com/tree-sitter/tree-sitter-rust",
            "revision": revision,
            "version": metadata.get("version"),
        },
        "corpusRoot": "test/corpus",
        "files": files,
    }
    return (json.dumps(profile, indent=2, sort_keys=True) + "\n").encode()


def expected_outputs(upstream: Path, provider_dir: Path) -> dict[Path, bytes]:
    upstream_queries = upstream / "queries"
    if not upstream_queries.is_dir():
        raise SystemExit(f"missing upstream queries dir: {upstream_queries}")

    tree_sitter_json = load_tree_sitter_json(upstream)
    grammar = (tree_sitter_json.get("grammars") or [{}])[0]
    declared_queries = {
        Path(path).name
        for field in ("highlights", "injections", "tags")
        for path in grammar.get(field, [])
    }
    expected_reference = set(UPSTREAM_REFERENCE_QUERY_FILES)
    if not expected_reference.issubset(declared_queries):
        raise SystemExit(
            "upstream tree-sitter.json no longer declares required reference queries: "
            f"expected at least {sorted(expected_reference)}, got {sorted(declared_queries)}"
        )

    return {
        **{
            provider_dir / "queries" / name: (upstream_queries / name).read_bytes()
            for name in UPSTREAM_REFERENCE_QUERY_FILES
        },
        provider_dir
        / CORPUS_PROFILE: build_corpus_profile(upstream, upstream_revision(upstream)),
    }


def check_provider_layout(provider_dir: Path) -> None:
    queries_dir = provider_dir / "queries"
    missing = [
        str(queries_dir / name)
        for name in (*UPSTREAM_REFERENCE_QUERY_FILES, *ASP_CATALOG_FILES)
        if not (queries_dir / name).is_file()
    ]
    if missing:
        raise SystemExit("missing provider query files:\n" + "\n".join(missing))

    stray_top_level = sorted(path.name for path in provider_dir.glob("*.scm"))
    if stray_top_level:
        raise SystemExit(
            "tree-sitter Rust queries must live under queries/: "
            + ", ".join(stray_top_level)
        )


def write_or_check(outputs: dict[Path, bytes], check: bool) -> list[str]:
    changed = []
    for path, expected in sorted(outputs.items()):
        current = path.read_bytes() if path.exists() else None
        if current == expected:
            continue
        changed.append(str(path))
        if not check:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(expected)
    return changed


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--upstream",
        required=True,
        type=Path,
        help="Path to a checked-out tree-sitter-rust repository.",
    )
    parser.add_argument(
        "--provider-dir",
        default=DEFAULT_PROVIDER_DIR,
        type=Path,
        help="Provider tree-sitter-rust catalog directory to update.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail if generated files differ instead of updating them.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    upstream = args.upstream.resolve()
    provider_dir = args.provider_dir.resolve()
    outputs = expected_outputs(upstream, provider_dir)
    changed = write_or_check(outputs, args.check)
    check_provider_layout(provider_dir)
    if changed and args.check:
        emit("tree-sitter Rust query snapshots are out of date:", file=sys.stderr)
        for path in changed:
            emit(f"  {path}", file=sys.stderr)
        return 1
    if changed:
        emit("updated tree-sitter Rust query snapshots:")
        for path in changed:
            emit(f"  {path}")
    else:
        emit("tree-sitter Rust query snapshots are up to date")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
