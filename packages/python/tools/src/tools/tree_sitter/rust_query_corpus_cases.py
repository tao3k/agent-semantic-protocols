"""Rust tree-sitter query corpus case parsing and validation."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path

from tools.paths import repo_root

_REPO_ROOT = repo_root()
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


@dataclass(frozen=True)
class _QueryCase:
    path: Path
    title: str
    catalog_id: str
    upstream_corpus: str
    fixture_case: str | None
    source: str
    expected_lines: list[str]


def _load_json(path: Path) -> dict:
    return json.loads(path.read_text())


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
        except json.JSONDecodeError:
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
