"""Tree-sitter JSON ABI corpus case parsing and packet assertions."""

from __future__ import annotations

import json
import re
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class _CorpusCase:
    language: str
    title: str
    catalog: str
    source_path: str
    source: str
    expected_captures: frozenset[tuple[str, str]]


def _load_cases(config: Any) -> list[_CorpusCase]:
    cases: list[_CorpusCase] = []
    for path in sorted(config.corpus_dir.glob("*.txt")):
        cases.extend(_parse_corpus_file(config, path))
    return cases


def _parse_corpus_file(config: Any, path: Path) -> list[_CorpusCase]:
    text = path.read_text(encoding="utf-8")
    blocks = [
        block.strip("\n")
        for block in re.split(r"^={20,}\n", text, flags=re.MULTILINE)
        if block.strip("\n")
    ]
    cases: list[_CorpusCase] = []
    for index in range(0, len(blocks) - 1, 2):
        metadata = blocks[index]
        payload = blocks[index + 1]
        title = metadata.splitlines()[0].strip()
        body, separator, expected = payload.partition("\n" + "-" * 80 + "\n")
        if not separator:
            continue
        catalog = _metadata_value(metadata, "catalog") or _metadata_value(
            body, "catalog"
        )
        if catalog is None:
            continue
        source_path = (
            _metadata_value(metadata, "file")
            or _metadata_value(body, "file")
            or config.default_source_path
        )
        expected_captures = frozenset(
            (match.group("name"), match.group("node"))
            for match in re.finditer(
                r'^capture\s+(?P<name>[^\s]+)\s+node=(?P<node>[^\s]+)\s+text="',
                expected,
                flags=re.MULTILINE,
            )
        )
        cases.append(
            _CorpusCase(
                language=config.language,
                title=title,
                catalog=catalog,
                source_path=source_path,
                source=_source_text(body),
                expected_captures=expected_captures,
            )
        )
    return cases


def _metadata_value(text: str, key: str) -> str | None:
    prefix = f"{key}:"
    for line in text.splitlines():
        if line.startswith(prefix):
            return line[len(prefix) :].strip()
    return None


def _source_text(body: str) -> str:
    if _metadata_value(body, "catalog") is not None:
        _metadata, separator, source = body.partition("\n\n")
        if not separator:
            return ""
        return source.strip("\n") + "\n"
    source = body
    return source.strip("\n") + "\n"


def _validate_case(
    case: _CorpusCase,
    *,
    asp_bin: str,
    repo_root: Path,
    optional_catalogs: frozenset[tuple[str, str]],
) -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        project_root = Path(temp_dir)
        source_path = project_root / case.source_path
        source_path.parent.mkdir(parents=True, exist_ok=True)
        source_path.write_text(case.source, encoding="utf-8")
        packet = _run_query(case, project_root, asp_bin=asp_bin, repo_root=repo_root)
    _assert_packet_shape(case, packet, optional_catalogs)


def _run_query(
    case: _CorpusCase,
    project_root: Path,
    *,
    asp_bin: str,
    repo_root: Path,
) -> dict[str, Any]:
    completed = subprocess.run(
        [
            asp_bin,
            case.language,
            "query",
            "--catalog",
            case.catalog,
            "--selector",
            case.source_path,
            "--json",
            str(project_root),
        ],
        cwd=repo_root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return json.loads(completed.stdout)


def _assert_packet_shape(
    case: _CorpusCase,
    packet: dict[str, Any],
    optional_catalogs: frozenset[tuple[str, str]],
) -> None:
    label = f"{case.language}:{case.title}:{case.catalog}"
    assert (
        packet["schemaId"] == "agent.semantic-protocols.semantic-tree-sitter-query"
    ), label
    assert packet["adapterMode"] == "native-projection", label
    assert packet["compatibilityLevel"] == "native-only", label
    assert packet["cache"]["rawSourceStored"] is False, label
    captures = [
        capture
        for match in packet["matches"]
        for capture in match.get("captures", [])
    ]
    if not captures:
        assert (case.language, case.catalog) in optional_catalogs, (
            f"{label}: expected at least one JSON capture"
        )
        _assert_compiled_plan_covers_corpus(case, packet)
        assert packet["nativeFactRefs"] == [], (
            f"{label}: empty native projection must not invent nativeFactRefs"
        )
        return
    assert packet["nativeFactRefs"], f"{label}: expected top-level nativeFactRefs"
    for capture in captures:
        _assert_capture_shape(label, capture)
    if case.expected_captures:
        actual_captures = {
            (capture["name"], capture["nodeType"]) for capture in captures
        }
        expected_namespaces = {
            name.split(".", maxsplit=1)[0]
            for name, _node_type in case.expected_captures
        }
        actual_namespaces = {
            name.split(".", maxsplit=1)[0] for name, _node_type in actual_captures
        }
        expected_node_types = {
            node_type for _name, node_type in case.expected_captures
        }
        actual_node_types = {node_type for _name, node_type in actual_captures}
        has_semantic_overlap = bool(
            (actual_captures & case.expected_captures)
            or (actual_namespaces & expected_namespaces)
            or (actual_node_types & expected_node_types)
        )
        assert has_semantic_overlap, (
            f"{label}: JSON captures do not align with corpus captures "
            f"actual={sorted(actual_captures)} expected={sorted(case.expected_captures)}"
        )


def _assert_compiled_plan_covers_corpus(
    case: _CorpusCase, packet: dict[str, Any]
) -> None:
    if not case.expected_captures:
        return
    label = f"{case.language}:{case.title}:{case.catalog}"
    query_fields = packet.get("query", {}).get("fields", {})
    plan_captures = set(query_fields.get("captures", []))
    plan_node_types = set(query_fields.get("nodeTypes", []))
    expected_capture_names = {name for name, _node in case.expected_captures}
    expected_node_types = {node for _name, node in case.expected_captures}
    assert expected_capture_names <= plan_captures, (
        f"{label}: compiled plan missing corpus captures "
        f"actual={sorted(plan_captures)} expected={sorted(expected_capture_names)}"
    )
    assert expected_node_types & plan_node_types, (
        f"{label}: compiled plan does not cover corpus node types "
        f"actual={sorted(plan_node_types)} expected={sorted(expected_node_types)}"
    )


def _assert_capture_shape(label: str, capture: dict[str, Any]) -> None:
    assert isinstance(capture.get("name"), str) and capture["name"], label
    assert isinstance(capture.get("nodeType"), str) and capture["nodeType"], label
    assert isinstance(capture.get("field"), str) and capture["field"], label
    assert capture.get("nativeFactRefs"), f"{label}: capture missing nativeFactRefs"
    fields = capture.get("fields")
    assert isinstance(fields, dict), f"{label}: capture fields missing"
    assert isinstance(fields.get("nativeNodeType"), str) and fields["nativeNodeType"], (
        f"{label}: capture missing nativeNodeType"
    )
