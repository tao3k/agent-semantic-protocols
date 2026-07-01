"""Language provider evidence graph and facade smoke tests."""

from __future__ import annotations

import json
import os
import re
import subprocess
from functools import lru_cache
from pathlib import Path
import time
from typing import Any

from unit._asp_graph_turbo_common import (
    _GRAPH_TURBO_REQUEST_SCHEMA,
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
)

_REPO_ROOT = Path(__file__).resolve().parents[2]
_EVIDENCE_GRAPH_SCHEMA = _REPO_ROOT / "schemas/semantic-evidence-graph.v1.schema.json"
_LANGUAGE_CASES = [
    ("rust", "languages/rust-lang-project-harness"),
    ("python", "languages/python-lang-project-harness"),
    ("typescript", "languages/typescript-lang-project-harness"),
    ("julia", "languages/JuliaLangProjectHarness.jl"),
    ("gerbil-scheme", "languages/gerbil-scheme-language-project-harness"),
]
_CORE_FAST_LANGUAGES = ("rust", "python", "typescript")
_ALL_PROVIDER_LANGUAGES = tuple(language for language, _ in _LANGUAGE_CASES)
_SYNCED_EVIDENCE_SCHEMAS = [
    "semantic-evidence-graph.v1.schema.json",
    "semantic-graph-turbo-request.v1.schema.json",
]
_TIMINGS: list[dict[str, Any]] = []


def test_active_language_evidence_smoke_matrix(tmp_path: Path) -> None:
    for language_id, package_root in _selected_language_cases():
        graph, request = _provider_evidence_packets(language_id, package_root, tmp_path)
        assert list(schema_validator_for(_EVIDENCE_GRAPH_SCHEMA).iter_errors(graph)) == []
        _assert_evidence_graph_packet(language_id, graph)
        assert list(schema_validator_for(_GRAPH_TURBO_REQUEST_SCHEMA).iter_errors(request)) == []
        _assert_graph_turbo_request(language_id, request)

        graph_turbo_packet = _rank_graph_turbo_request(request)
        assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(graph_turbo_packet)) == []
        assert graph_turbo_packet["schemaId"] == "agent.semantic-protocols.semantic-graph-turbo-result"
        assert graph_turbo_packet["profile"] == request["profile"]

        guide_or_help = _provider_evidence_guide(language_id, package_root)
        assert "evidence graph" in guide_or_help or "evidence-graph" in guide_or_help
        assert "evidence analyze" in guide_or_help or "evidence-analyze" in guide_or_help


def test_active_language_facade_conformance_matrix() -> None:
    for language_id, package_root in _selected_language_cases():
        guide = _provider_guide(language_id, package_root)
        assert f"asp {language_id} search prime" in guide
        assert f"asp {language_id} search pipe" in guide
        assert f"asp {language_id} query" in guide
        assert "--selector" in guide
        assert "--code" in guide
        assert "evidence graph" in guide or "evidence-graph" in guide
        assert "evidence analyze" in guide or "evidence-analyze" in guide

        pipe_output = _run_asp_text(
            language_id,
            "search",
            "pipe",
            "facade conformance guide selector evidence",
            "--workspace",
            package_root,
            "--view",
            "seeds",
        )
        _assert_search_pipe_quality(language_id, pipe_output)


def test_language_evidence_timing_receipt() -> None:
    receipt = _timing_receipt()
    _write_timing_receipt(receipt)

    slow = [
        timing
        for timing in receipt["timings"]
        if timing["durationSeconds"] > timing["maxCommandSeconds"]
    ]
    assert slow == [], (
        f"language evidence smoke command exceeded timing threshold: {slow}"
    )


def test_language_package_evidence_schemas_are_synced() -> None:
    for _, package_root in _LANGUAGE_CASES:
        schema_dir = _REPO_ROOT / package_root / "schemas"
        for schema_name in _SYNCED_EVIDENCE_SCHEMAS:
            root_schema = _REPO_ROOT / "schemas" / schema_name
            package_schema = schema_dir / schema_name
            assert package_schema.exists(), f"missing package-local schema: {package_schema}"
            assert package_schema.read_bytes() == root_schema.read_bytes(), (
                f"schema drift: {package_schema} differs from {root_schema}"
            )


def _provider_evidence_packets(
    language_id: str,
    package_root: str,
    tmp_path: Path,
) -> tuple[dict[str, Any], dict[str, Any]]:
    if language_id == "rust":
        review_packet_path = tmp_path / "rust-review-packet.json"
        review_packet_path.write_text(json.dumps(_rust_review_packet()), encoding="utf-8")
        graph = _run_asp_json(
            "rust",
            "evidence",
            "graph",
            "--review-packet-json",
            str(review_packet_path),
            "--json",
            package_root,
        )
        graph_path = tmp_path / "rust-evidence-graph.json"
        graph_path.write_text(json.dumps(graph), encoding="utf-8")
        request = _run_asp_json(
            "rust",
            "evidence",
            "analyze",
            "--evidence-graph-json",
            str(graph_path),
            "--json",
            package_root,
        )
        return graph, request

    graph = _run_asp_json(language_id, "evidence", "graph", "--json", package_root)
    request = _run_asp_json(language_id, "evidence", "analyze", "--json", package_root)
    return graph, request


def _provider_evidence_guide(language_id: str, package_root: str) -> str:
    if language_id == "rust":
        return _run_asp_text("rust", "evidence", "--help")
    return _provider_guide(language_id, package_root)


@lru_cache(maxsize=None)
def _provider_guide(language_id: str, package_root: str) -> str:
    return _run_asp_text(language_id, "guide", package_root)


def _rank_graph_turbo_request(request: dict[str, Any]) -> dict[str, Any]:
    graph = TypedGraph.from_packet(request)
    result = rank_frontier(
        graph,
        profile=str(request["profile"]),
        seeds=[str(seed) for seed in request.get("seedIds", [])],
        limit=8,
        cache_enabled=False,
    )
    return result_to_packet(result)


def _assert_evidence_graph_packet(language_id: str, graph: dict[str, Any]) -> None:
    assert graph["schemaId"] == "agent.semantic-protocols.semantic-evidence-graph"
    assert graph["producer"]["languageId"] == language_id
    assert graph.get("nodes"), f"{language_id} evidence graph returned no nodes"
    assert graph.get("edges"), f"{language_id} evidence graph returned no edges"


def _assert_graph_turbo_request(language_id: str, request: dict[str, Any]) -> None:
    assert request["schemaId"] == "agent.semantic-protocols.semantic-graph-turbo-request"
    assert request["profile"] in {"evidence-quality", f"{language_id}-evidence-quality"}
    assert request["producer"]["languageId"] == language_id
    assert request.get("seedIds"), f"{language_id} graph-turbo request returned no seeds"
    assert request.get("graphs"), f"{language_id} graph-turbo request returned no graphs"
    summary = request.get("summary", {})
    assert summary.get("nodes", 0) > 0, f"{language_id} graph-turbo request returned no nodes"
    assert summary.get("edges", 0) > 0, f"{language_id} graph-turbo request returned no edges"


def _assert_search_pipe_quality(language_id: str, pipe_output: str) -> None:
    assert pipe_output.strip()
    assert "[search-pipe]" in pipe_output, f"{language_id} pipe did not use facade pipe"
    assert "queryQuality=" in pipe_output, f"{language_id} pipe missing query quality"
    assert (
        "actionFrontier=" in pipe_output or "nextCommand=" in pipe_output
    ), f"{language_id} pipe missing next action guidance"
    assert (
        "recommendedNext=" in pipe_output or "nextCommand=" in pipe_output
    ), f"{language_id} pipe missing recommendation"
    assert "avoid=" in pipe_output, f"{language_id} pipe missing avoid guidance"
    assert (
        "raw-read" in pipe_output or "direct-source-read" in pipe_output
    ), f"{language_id} pipe missing source-read avoidance"


def _run_asp_json(*args: str) -> dict[str, Any]:
    output = _run_asp_text(*args)
    return json.loads(output)


def _run_asp_text(*args: str) -> str:
    started_at = time.perf_counter()
    completed = subprocess.run(
        ["asp", *args],
        cwd=_REPO_ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    _TIMINGS.append(
        {
            "language": _timing_language(args),
            "command": _timing_command(args),
            "durationSeconds": round(time.perf_counter() - started_at, 3),
        }
    )
    return completed.stdout


@lru_cache(maxsize=1)
def _active_asp_languages() -> frozenset[str]:
    providers = _run_asp_text("providers")
    return frozenset(
        match.group(1)
        for match in re.finditer(r"\|provider language=([^ ]+)", providers)
        if match.group(1) not in {"org", "md"}
    )


@lru_cache(maxsize=1)
def _selected_language_cases() -> tuple[tuple[str, str], ...]:
    active_languages = _active_asp_languages()
    requested_languages = _requested_language_ids(active_languages)
    known_languages = set(_ALL_PROVIDER_LANGUAGES)
    unknown_languages = sorted(set(requested_languages) - known_languages)
    assert unknown_languages == [], f"unknown language evidence smoke ids: {unknown_languages}"

    missing_languages = sorted(set(requested_languages) - active_languages)
    assert missing_languages == [], (
        f"requested language evidence smoke providers are not active: {missing_languages}; "
        "run the setup target or adjust ASP_LANGUAGE_EVIDENCE_LANGUAGES"
    )

    return tuple(
        (language, package_root)
        for language, package_root in _LANGUAGE_CASES
        if language in requested_languages
    )


def _requested_language_ids(active_languages: frozenset[str]) -> tuple[str, ...]:
    explicit = os.environ.get("ASP_LANGUAGE_EVIDENCE_LANGUAGES", "").strip()
    if explicit:
        return tuple(language.strip() for language in explicit.split(",") if language.strip())

    scope = os.environ.get("ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE", "core-fast").strip()
    if scope == "core-fast":
        return _CORE_FAST_LANGUAGES
    if scope == "all-providers":
        return _ALL_PROVIDER_LANGUAGES
    if scope == "active":
        return tuple(language for language in _ALL_PROVIDER_LANGUAGES if language in active_languages)

    raise AssertionError(
        "ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE must be core-fast, all-providers, or active"
    )


def _max_command_seconds() -> float:
    explicit = os.environ.get("ASP_LANGUAGE_EVIDENCE_MAX_COMMAND_SECONDS")
    if explicit:
        return float(explicit)
    return 10.0


def _julia_max_command_seconds() -> float:
    explicit = os.environ.get("ASP_LANGUAGE_EVIDENCE_MAX_COMMAND_SECONDS_JULIA")
    if explicit:
        return float(explicit)
    return 2.0


def _timing_max_command_seconds(timing: dict[str, Any]) -> float:
    if timing["language"] == "julia":
        return _julia_max_command_seconds()
    return _max_command_seconds()


def _timing_language(args: tuple[str, ...]) -> str:
    if args and args[0] in _ALL_PROVIDER_LANGUAGES:
        return args[0]
    return "asp"


def _timing_command(args: tuple[str, ...]) -> str:
    if len(args) >= 3 and args[1] in {"search", "evidence"}:
        return f"{args[1]} {args[2]}"
    if len(args) >= 2:
        return args[1]
    return " ".join(args)


def _timing_receipt() -> dict[str, Any]:
    active_languages = _active_asp_languages()
    requested_languages = tuple(language for language, _ in _selected_language_cases())
    timings = [
        {
            **timing,
            "maxCommandSeconds": _timing_max_command_seconds(timing),
        }
        for timing in _TIMINGS
    ]
    return {
        "schemaId": "agent.semantic-protocols.language-evidence-smoke-timing",
        "schemaVersion": "1",
        "scope": os.environ.get("ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE", "core-fast").strip(),
        "requestedLanguages": list(requested_languages),
        "activeLanguages": sorted(active_languages),
        "maxCommandSeconds": _max_command_seconds(),
        "languageMaxCommandSeconds": {"julia": _julia_max_command_seconds()},
        "timings": timings,
    }


def _write_timing_receipt(receipt: dict[str, Any]) -> None:
    path_text = os.environ.get("ASP_LANGUAGE_EVIDENCE_TIMING_JSON", "").strip()
    if not path_text:
        return
    path = Path(path_text)
    if not path.is_absolute():
        path = _REPO_ROOT / path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _rust_review_packet() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-review-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.review-packet",
        "protocolVersion": "1",
        "packetId": "rust.review.packet",
        "producer": {
            "languageId": "rust",
            "providerId": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        },
        "project": {"root": "."},
        "summary": {
            "changedInvariants": 1,
            "changedBehavior": 1,
            "missingReceipts": 1,
            "staleWaivers": 1,
            "determinismObservations": 0,
            "proofClaims": 0,
        },
        "changedInvariants": [
            {
                "invariantId": "agent-r027:src.model.rs:42",
                "sourceRuleId": "AGENT-R027",
                "kind": "public-data-primitive-fields",
                "severity": "warning",
                "title": "semantic fields need named type",
                "hypothesis": "public data shape should not expose stringly fields",
                "location": {"path": "src/model.rs", "line": 42, "column": 0},
                "requiredReceipts": ["cargo-check", "expect-test"],
            }
        ],
        "changedBehavior": [
            {
                "snapshotId": "rust.behavior.src-model",
                "status": "changed",
                "subject": "src/model.rs",
                "summary": "expect-test output changed",
                "receiptIds": ["rust.expect-test.src-model"],
                "candidateIds": ["agent-r027:src.model.rs:42"],
            }
        ],
        "missingReceipts": [
            {
                "invariantId": "agent-r027:src.model.rs:42",
                "receiptKind": "expect-test",
                "reason": "no passed expect-test receipt linked to candidate",
            }
        ],
        "staleWaivers": [
            {
                "waiverId": "waiver.agent-r027.src-model",
                "invariantId": "agent-r027:src.model.rs:42",
                "receiptKind": "expect-test",
                "status": "stale",
                "owner": "reviewer",
                "reason": "snapshot migration is pending",
            }
        ],
        "reviewActions": [
            {
                "actionId": "run-receipt.agent-r027.src-model.expect-test",
                "kind": "run-receipt",
                "priority": "p0",
                "summary": "Run expect-test for agent-r027:src.model.rs:42",
                "targetId": "agent-r027:src.model.rs:42",
            }
        ],
    }
