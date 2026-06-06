"""Offline artifact conversion and evaluation for graph turbo."""

from __future__ import annotations

import json
import time
from collections import Counter
from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .artifact_commands import command_artifact_dir, command_labels_by_language
from .artifact_coverage import ArtifactRankCase, label_coverage
from .artifact_requests import search_artifact_dir, search_packet_to_graph_turbo_request
from .artifact_targets import search_packet_input_targets
from .model import GraphResult, TypedGraph
from .ranking import rank_frontier


@dataclass(frozen=True)
class _ConvertedArtifact:
    path: Path
    packet: Mapping[str, Any]
    request: Mapping[str, object]
    result: GraphResult
    second_pass_cache_status: str
    input_targets: tuple[str, ...]
    output_targets: tuple[str, ...]


@dataclass
class _EvaluationState:
    scanned: int = 0
    converted: int = 0
    skipped: int = 0
    second_pass_hits: int = 0
    profile_counts: Counter[str] = field(default_factory=Counter)
    method_counts: Counter[str] = field(default_factory=Counter)
    language_counts: Counter[str] = field(default_factory=Counter)
    aggregate: Counter[str] = field(default_factory=Counter)
    example_rows: list[dict[str, object]] = field(default_factory=list)
    rank_cases: list[ArtifactRankCase] = field(default_factory=list)


def evaluate_search_artifacts(
    root: Path,
    *,
    limit: int | None = None,
    budget: int = 10,
    examples: int = 5,
) -> dict[str, object]:
    search_dir = search_artifact_dir(root)
    started = time.perf_counter()
    state = _evaluate_artifact_paths(search_dir, limit=limit, budget=budget, examples=examples)
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    return _evaluation_packet(root, search_dir, state, elapsed_ms, examples)


def _evaluate_artifact_paths(
    search_dir: Path,
    *,
    limit: int | None,
    budget: int,
    examples: int,
) -> _EvaluationState:
    state = _EvaluationState()
    for path in _search_artifact_paths(search_dir, limit):
        state.scanned += 1
        converted_artifact = _converted_artifact(path, budget=budget)
        if converted_artifact is None:
            state.skipped += 1
            continue
        state.converted += 1
        if converted_artifact.second_pass_cache_status == "hit":
            state.second_pass_hits += 1
        language = _record_converted_artifact(
            converted_artifact,
            state.profile_counts,
            state.method_counts,
            state.language_counts,
            state.aggregate,
            state.rank_cases,
        )
        if len(state.example_rows) < examples:
            state.example_rows.append(_example_row(converted_artifact, language))
    return state


def _evaluation_packet(
    root: Path,
    search_dir: Path,
    state: _EvaluationState,
    elapsed_ms: int,
    examples: int,
) -> dict[str, object]:
    label_dir = command_artifact_dir(root)
    labels = command_labels_by_language(label_dir)
    return {
        "schemaId": "agent.semantic-protocols.graph-turbo-artifact-eval",
        "schemaVersion": "1",
        "artifactDir": str(search_dir),
        "labelSourceDir": str(label_dir) if label_dir is not None else None,
        "scanned": state.scanned,
        "converted": state.converted,
        "skipped": state.skipped,
        "elapsedMs": elapsed_ms,
        "secondPassCacheHits": state.second_pass_hits,
        "profileCounts": dict(state.profile_counts),
        "methodCounts": dict(state.method_counts),
        "languageCounts": dict(state.language_counts),
        "averages": _averages(state.aggregate, state.converted),
        "historicalCommandCoverage": label_coverage(
            state.rank_cases,
            labels,
            examples=examples,
        ),
        "examples": state.example_rows,
    }


def _converted_artifact(path: Path, *, budget: int) -> _ConvertedArtifact | None:
    packet = _load_json(path)
    request = search_packet_to_graph_turbo_request(packet, budget=budget)
    if request is None:
        return None
    result, second = _rank_request_twice(request)
    return _ConvertedArtifact(
        path=path,
        packet=packet,
        request=request,
        result=result,
        second_pass_cache_status=second.graph_cache.status,
        input_targets=search_packet_input_targets(packet),
        output_targets=tuple(node.value for node in result.ranked_nodes),
    )


def _record_converted_artifact(
    artifact: _ConvertedArtifact,
    profile_counts: Counter[str],
    method_counts: Counter[str],
    language_counts: Counter[str],
    aggregate: Counter[str],
    rank_cases: list[ArtifactRankCase],
) -> str:
    language = str(artifact.packet.get("languageId") or "unknown")
    profile_counts[str(artifact.request["profile"])] += 1
    method_counts[str(artifact.packet.get("method") or "unknown")] += 1
    language_counts[language] += 1
    rank_cases.append(
        ArtifactRankCase(
            language=language,
            path=str(artifact.path),
            input_targets=artifact.input_targets,
            ranked_values=artifact.output_targets,
        )
    )
    aggregate.update(_aggregate_row(artifact))
    return language


def _aggregate_row(artifact: _ConvertedArtifact) -> dict[str, int]:
    return {
        "inputTargets": len(artifact.input_targets),
        "rankedNodes": len(artifact.result.ranked_nodes),
        "selectedEdges": len(artifact.result.selected_edges),
        "typedPaths": len(artifact.result.typed_paths),
        "inputMaxDuplicate": _max_duplicate(artifact.input_targets),
        "rankedMaxDuplicate": _max_duplicate(artifact.output_targets),
    }


def _example_row(artifact: _ConvertedArtifact, language: str) -> dict[str, object]:
    return {
        "path": str(artifact.path),
        "language": language,
        "method": artifact.packet.get("method") or "unknown",
        "profile": artifact.request["profile"],
        "inputTargets": len(artifact.input_targets),
        "rankedNodes": [node.id for node in artifact.result.ranked_nodes],
        "inputMaxDuplicate": _max_duplicate(artifact.input_targets),
        "rankedMaxDuplicate": _max_duplicate(artifact.output_targets),
        "pathCount": len(artifact.result.typed_paths),
        "cacheStatus2": artifact.second_pass_cache_status,
    }


def _rank_request_twice(request: Mapping[str, object]):
    graph = TypedGraph.from_packet(request)
    kwargs = {
        "profile": str(request["profile"]),
        "seeds": tuple(str(item) for item in request["seedIds"]),
        "limit": int(request["budget"]),
        "kind_budgets": request["kindBudgets"],
        "path_budget": int(request["pathBudget"]),
        "path_max_hops": int(request["pathMaxHops"]),
    }
    first = rank_frontier(graph, **kwargs)
    second = rank_frontier(graph, **kwargs)
    return first, second


def _search_artifact_paths(root: Path, limit: int | None) -> Iterable[Path]:
    count = 0
    for path in sorted(root.glob("*.json")):
        yield path
        count += 1
        if limit is not None and count >= limit:
            return


def _load_json(path: Path) -> Mapping[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    return value if isinstance(value, Mapping) else {}


def _max_duplicate(values: Iterable[str]) -> int:
    counts = Counter(values)
    return max(counts.values(), default=0)


def _averages(values: Counter[str], count: int) -> dict[str, float]:
    if count <= 0:
        return {}
    return {key: round(value / count, 2) for key, value in sorted(values.items())}
