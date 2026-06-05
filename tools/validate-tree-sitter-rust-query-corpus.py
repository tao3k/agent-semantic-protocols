#!/usr/bin/env python3
"""Validate Rust ASP tree-sitter query corpus capture contracts."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[1]
_PROVIDER_DIR = (
    _REPO_ROOT / "languages/rust-lang-project-harness/tree-sitter/tree-sitter-rust"
)
_SECTION_LINE = "=" * 80
_DIVIDER_LINE = "-" * 80
_CASE_RE = re.compile(
    rf"^{_SECTION_LINE}\n(?P<header>.*?)\n{_SECTION_LINE}\n(?P<body>.*?)(?=\n{_SECTION_LINE}\n|\Z)",
    re.MULTILINE | re.DOTALL,
)
_CAPTURE_RE = re.compile(
    r'^capture (?P<capture>\S+) node=(?P<node>[A-Za-z_][A-Za-z0-9_]*|_) '
    r'text="(?P<text>.*)"$'
)
_NODE_RE = re.compile(r"\(([A-Za-z_][A-Za-z0-9_]*)")
_CAPTURE_NAME_RE = re.compile(r"@([A-Za-z0-9_.-]+)")
_GIT_REV_RE = re.compile(r"^[0-9a-f]{40}$")


@dataclass(frozen=True)
class _QueryCase:
    path: Path
    title: str
    catalog_id: str
    upstream_corpus: str
    fixture_case: str | None
    source: str
    expected_lines: list[str]


def _catalogs(profile: dict) -> dict[str, dict]:
    return {catalog["id"]: catalog for catalog in profile.get("catalogs", [])}


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def _current_asp_revision() -> str:
    return subprocess.check_output(
        ["git", "-C", str(_REPO_ROOT), "rev-parse", "HEAD"],
        text=True,
        stderr=subprocess.DEVNULL,
    ).strip()


def _validate_asp_workspace_profile(
    profile: dict,
    check_current_revision: bool,
) -> list[str]:
    workspace = profile.get("aspWorkspace")
    if not isinstance(workspace, dict):
        return ["grammar-profile.json: missing aspWorkspace provenance"]

    errors = []
    if workspace.get("owner") != "main-asp":
        errors.append("grammar-profile.json: aspWorkspace.owner must be main-asp")
    if not workspace.get("repository"):
        errors.append("grammar-profile.json: aspWorkspace.repository is required")
    revision = workspace.get("revision")
    if not isinstance(revision, str) or not _GIT_REV_RE.match(revision):
        errors.append("grammar-profile.json: aspWorkspace.revision must be a 40-hex git rev")
    if workspace.get("queryCorpusValidator") != "tools/validate-tree-sitter-rust-query-corpus.py":
        errors.append("grammar-profile.json: aspWorkspace.queryCorpusValidator is stale")
    if check_current_revision and isinstance(revision, str):
        current_revision = _current_asp_revision()
        if revision != current_revision:
            errors.append(
                "grammar-profile.json: aspWorkspace.revision does not match current ASP "
                f"HEAD {current_revision}"
            )
    return errors


def _parse_case(path: Path, header_text: str, body_text: str) -> _QueryCase:
    header = header_text.strip("\n").splitlines()
    body = body_text.strip("\n").splitlines()
    if not header:
        raise ValueError(f"{path}: malformed case header")
    try:
        divider = body.index(_DIVIDER_LINE)
    except ValueError as error:
        raise ValueError(f"{path}: missing expectation divider") from error

    title = header[0].strip()
    metadata = {}
    for line in header[1:]:
        key, separator, value = line.partition(":")
        if not separator:
            raise ValueError(f"{path}: malformed metadata line: {line}")
        metadata[key.strip()] = value.strip()

    catalog_id = metadata.get("catalog")
    upstream_corpus = metadata.get("upstream-corpus")
    if not catalog_id or not upstream_corpus:
        raise ValueError(f"{path}: missing catalog or upstream-corpus metadata")

    return _QueryCase(
        path=path,
        title=title,
        catalog_id=catalog_id,
        upstream_corpus=upstream_corpus,
        fixture_case=metadata.get("fixture-case"),
        source="\n".join(body[:divider]).strip("\n"),
        expected_lines=[line for line in body[divider + 1 :] if line.strip()],
    )


def _parse_cases(path: Path) -> list[_QueryCase]:
    text = path.read_text()
    cases = [
        _parse_case(path, match.group("header"), match.group("body"))
        for match in _CASE_RE.finditer(text)
    ]
    if not cases:
        raise ValueError(f"{path}: no query corpus cases found")
    return cases


def _query_nodes(query_source: str) -> set[str]:
    return set(_NODE_RE.findall(query_source))


def _query_captures(query_source: str) -> set[str]:
    return set(_CAPTURE_NAME_RE.findall(query_source))


def _validate_case(
    case: _QueryCase,
    provider_dir: Path,
    catalogs: dict[str, dict],
    upstream_corpus_paths: set[str],
    allowed_fixture_cases: set[str],
) -> list[str]:
    errors = []
    catalog = catalogs.get(case.catalog_id)
    if catalog is None:
        return [f"{case.path}: {case.title}: unknown catalog {case.catalog_id}"]
    if case.upstream_corpus not in upstream_corpus_paths:
        errors.append(
            f"{case.path}: {case.title}: unknown upstream corpus {case.upstream_corpus}"
        )

    query_path = provider_dir / Path(catalog["path"]).relative_to(
        "tree-sitter/tree-sitter-rust"
    )
    query_source = query_path.read_text()
    query_nodes = _query_nodes(query_source)
    query_captures = _query_captures(query_source)
    declared_captures = set(catalog.get("captures", []))
    source_text = case.source
    if case.fixture_case:
        if case.fixture_case not in allowed_fixture_cases:
            errors.append(
                f"{case.path}: {case.title}: fixture-case not listed in grammar profile"
            )
        fixture_path = _REPO_ROOT / case.fixture_case
        try:
            fixture = _load_json(fixture_path)
        except FileNotFoundError:
            errors.append(f"{case.path}: {case.title}: missing fixture case {fixture_path}")
            fixture = {}
        if fixture.get("featureClass") != "real-library":
            errors.append(f"{case.path}: {case.title}: fixture-case must be real-library")
        if fixture.get("languageId") != "rust":
            errors.append(f"{case.path}: {case.title}: fixture-case must be Rust")
        fixture_root = fixture.get("fixtureRoot")
        raw_source_path = fixture.get("rawSourcePath")
        if isinstance(fixture_root, str) and isinstance(raw_source_path, str):
            raw_source = _REPO_ROOT / fixture_root / raw_source_path
            try:
                source_text = raw_source.read_text()
            except FileNotFoundError:
                errors.append(f"{case.path}: {case.title}: missing raw fixture {raw_source}")
        else:
            errors.append(f"{case.path}: {case.title}: fixture-case lacks raw source path")

    for line in case.expected_lines:
        match = _CAPTURE_RE.match(line)
        if match is None:
            errors.append(f"{case.path}: {case.title}: malformed expected line: {line}")
            continue
        capture = match.group("capture")
        node = match.group("node")
        try:
            text = json.loads(f'"{match.group("text")}"')
        except json.JSONDecodeError as error:
            errors.append(f"{case.path}: {case.title}: malformed text string: {line}")
            continue
        if capture not in declared_captures:
            errors.append(f"{case.path}: {case.title}: undeclared capture {capture}")
        if capture not in query_captures:
            errors.append(f"{case.path}: {case.title}: capture not in query {capture}")
        if node != "_" and node not in query_nodes:
            errors.append(f"{case.path}: {case.title}: node not in query {node}")
        if text not in source_text:
            errors.append(f"{case.path}: {case.title}: text not in source: {text}")
    return errors


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--provider-dir",
        default=_PROVIDER_DIR,
        type=Path,
        help="Provider tree-sitter-rust catalog directory.",
    )
    parser.add_argument(
        "--check-current-asp-revision",
        action="store_true",
        help="Also require aspWorkspace.revision to match the current ASP checkout HEAD.",
    )
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    provider_dir = args.provider_dir.resolve()
    profile = _load_json(provider_dir / "grammar-profile.json")
    corpus_profile = _load_json(provider_dir / "corpus-profile.json")
    errors = _validate_asp_workspace_profile(
        profile,
        args.check_current_asp_revision,
    )
    query_corpus = profile.get("queryCorpus", {})
    if query_corpus.get("path") != "tree-sitter/tree-sitter-rust/query-corpus":
        errors.append("grammar-profile.json: queryCorpus.path is stale")
    if query_corpus.get("validator") != "tools/validate-tree-sitter-rust-query-corpus.py":
        errors.append("grammar-profile.json: queryCorpus.validator is stale")
    allowed_fixture_cases = set(query_corpus.get("realLibraryCases", []))
    upstream_corpus_paths = {
        item["path"] for item in corpus_profile.get("files", [])
    }
    catalogs = _catalogs(profile)
    corpus_dir = provider_dir / "query-corpus"
    for path in sorted(corpus_dir.glob("*.txt")):
        for case in _parse_cases(path):
            errors.extend(
                _validate_case(
                    case,
                    provider_dir,
                    catalogs,
                    upstream_corpus_paths,
                    allowed_fixture_cases,
                )
            )
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("tree-sitter Rust query corpus is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
