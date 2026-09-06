#!/usr/bin/env python3
# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Sync tree-sitter query snapshots from an upstream checkout.

This is a development/CI maintenance tool. Runtime providers still embed
committed query files into the binary/package artifact.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

from tools.console import emit

DEFAULT_EXCLUDED_QUERY_FILES = ("highlights.scm",)
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
        return json.loads(path.read_text(encoding="utf-8"))
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
    links = metadata.get("links", {})
    repository = links.get("repository") if isinstance(links, dict) else None
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
            "repository": repository or f"https://github.com/tree-sitter/{upstream.name}",
            "revision": revision,
            "version": metadata.get("version"),
        },
        "corpusRoot": "test/corpus",
        "files": files,
    }
    return (json.dumps(profile, indent=2, sort_keys=True) + "\n").encode()


def expected_outputs(
    upstream: Path,
    provider_dir: Path,
    *,
    include: set[str] | None = None,
    exclude: set[str] | None = None,
    corpus_profile: bool = True,
) -> dict[Path, bytes]:
    upstream_queries = upstream / "queries"
    if not upstream_queries.is_dir():
        raise SystemExit(f"missing upstream queries dir: {upstream_queries}")

    excluded = set(DEFAULT_EXCLUDED_QUERY_FILES)
    if exclude:
        excluded.update(exclude)
    query_files = []
    for path in sorted(upstream_queries.glob("*.scm")):
        if path.name in excluded:
            continue
        if include is not None and path.name not in include:
            continue
        query_files.append(path)

    if not query_files:
        raise SystemExit(
            "no upstream query snapshots selected; "
            f"include={sorted(include) if include is not None else 'all'} "
            f"exclude={sorted(excluded)}"
        )

    outputs = {
        provider_dir / "queries" / path.name: path.read_bytes() for path in query_files
    }
    if corpus_profile:
        outputs[provider_dir / CORPUS_PROFILE] = build_corpus_profile(
            upstream,
            upstream_revision(upstream),
        )
    return outputs


def check_no_highlights(provider_dir: Path) -> None:
    highlights = provider_dir / "queries" / "highlights.scm"
    if highlights.exists():
        raise SystemExit(
            "highlights.scm is intentionally excluded from ASP syntax query catalogs: "
            f"{highlights}"
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


def parse_args(
    argv: list[str],
    *,
    default_provider_dir: Path | None = None,
) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--upstream",
        required=True,
        type=Path,
        help="Path to a checked-out tree-sitter grammar repository.",
    )
    parser.add_argument(
        "--provider-dir",
        default=default_provider_dir,
        required=default_provider_dir is None,
        type=Path,
        help="Provider grammar directory to update, such as tree-sitter/tree-sitter-rust.",
    )
    parser.add_argument(
        "--include",
        action="append",
        default=None,
        help="Specific upstream query file to sync. May be repeated. Defaults to all .scm files except highlights.scm.",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        help="Additional upstream query file to skip. highlights.scm is always skipped.",
    )
    parser.add_argument(
        "--no-corpus-profile",
        action="store_true",
        help="Do not generate corpus-profile.json from upstream test/corpus.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail if generated files differ instead of updating them.",
    )
    return parser.parse_args(argv)


def main(
    argv: list[str] | None = None,
    *,
    default_provider_dir: Path | None = None,
    output_label: str = "query",
) -> int:
    args = parse_args(
        list(sys.argv[1:] if argv is None else argv),
        default_provider_dir=default_provider_dir,
    )
    upstream = args.upstream.resolve()
    provider_dir = args.provider_dir.resolve()
    include = set(args.include) if args.include else None
    outputs = expected_outputs(
        upstream,
        provider_dir,
        include=include,
        exclude=set(args.exclude),
        corpus_profile=not args.no_corpus_profile,
    )
    changed = write_or_check(outputs, args.check)
    check_no_highlights(provider_dir)
    if changed and args.check:
        emit("tree-sitter query snapshots are out of date:", file=sys.stderr)
        for path in changed:
            emit(f"  {path}", file=sys.stderr)
        return 1
    if changed:
        emit(f"updated tree-sitter {output_label} snapshots:")
        for path in changed:
            emit(f"  {path}")
    else:
        emit(f"tree-sitter {output_label} snapshots are up to date")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
