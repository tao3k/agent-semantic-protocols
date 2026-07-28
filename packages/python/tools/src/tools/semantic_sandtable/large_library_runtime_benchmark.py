"""Orchestrate a complete public-ASP benchmark across pinned large libraries."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .large_library_runtime_deployment import (
    install_workspace_providers,
    release_binary_is_valid,
)
from .large_library_runtime_artifact import resolve_corpora
from .large_library_runtime_manifest import (
    corpus_from_manifest,
    validate_corpus_scenario,
)
from .large_library_runtime_registry import search_descriptors
from .large_library_runtime_receipt import coverage, empty_coverage, runtime_receipt
from .large_library_runtime_steps import benchmark_fd_step, benchmark_step, warmup
from .large_library_runtime_types import Corpus
from .scenario_io import discover_scenarios, load_scenario
from .utils import string_list


_CORPUS_MANIFEST = "benchmarks/large-library-runtime-corpora.v1.json"
LIVE_CORPUS_LANGUAGES = (
    "gerbil-scheme",
    "julia",
    "md",
    "org",
    "python",
    "rust",
    "typescript",
)


def run_large_library_runtime_benchmark(
    repo_root: Path,
    *,
    asp_binary: Path,
    state_home: Path,
    languages: tuple[str, ...] = LIVE_CORPUS_LANGUAGES,
) -> dict[str, Any]:
    """Install live providers and execute every registered search method."""
    selected_languages = tuple(sorted(set(languages)))
    corpora = load_corpora(repo_root, selected_languages)
    binary = asp_binary.expanduser().resolve()
    release_verified = release_binary_is_valid(binary)
    resolved, missing = resolve_corpora(corpora, state_home)
    empty = empty_coverage()
    if not release_verified or missing:
        return runtime_receipt(
            binary=binary,
            release_verified=release_verified,
            workspace_deployments=[],
            corpora=resolved,
            missing=missing,
            command_coverage=empty,
            warmups=[],
            steps=[],
        )

    workspace_deployments = install_workspace_providers(
        binary,
        repo_root,
        tuple(sorted({corpus.language for corpus in corpora})),
    )
    if any(deployment["status"] != "pass" for deployment in workspace_deployments):
        return runtime_receipt(
            binary=binary,
            release_verified=True,
            workspace_deployments=workspace_deployments,
            corpora=resolved,
            missing=[],
            command_coverage=empty,
            warmups=[],
            steps=[],
        )

    steps, warmups, registered_methods, command_count = execute_corpora(
        binary, corpora, resolved
    )
    return runtime_receipt(
        binary=binary,
        release_verified=True,
        workspace_deployments=workspace_deployments,
        corpora=resolved,
        missing=[],
        command_coverage=coverage(registered_methods, command_count, steps),
        warmups=warmups,
        steps=steps,
    )


def execute_corpora(
    binary: Path,
    corpora: list[Corpus],
    resolved_corpora: list[dict[str, str]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], set[str], int]:
    steps: list[dict[str, Any]] = []
    warmups: list[dict[str, Any]] = []
    registered_methods: set[str] = set()
    command_count = 0
    workspaces = {
        record["scenarioId"]: Path(record["path"]) for record in resolved_corpora
    }
    for corpus in corpora:
        workspace = workspaces[corpus.scenario_id]
        descriptors, registry_error = search_descriptors(binary, corpus, workspace)
        if registry_error is not None:
            steps.append(registry_error)
            continue
        methods = {str(descriptor["method"]) for descriptor in descriptors}
        registered_methods.update(methods)
        registered_methods.add("search/fd-path")
        command_count += len(descriptors) + 1
        warmups.append(warmup(binary, corpus, workspace, descriptors))
        steps.append(benchmark_fd_step(binary, corpus, workspace))
        steps.extend(
            benchmark_step(binary, corpus, workspace, descriptor)
            for descriptor in descriptors
        )
    return steps, warmups, registered_methods, command_count


def load_corpora(repo_root: Path, languages: tuple[str, ...]) -> list[Corpus]:
    manifest_path = repo_root / _CORPUS_MANIFEST
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if (
        manifest.get("schemaId")
        != "agent.semantic-protocols.semantic-sandtable-large-library-corpora"
        or manifest.get("schemaVersion") != "1"
    ):
        raise ValueError(f"invalid large-library corpus manifest: {manifest_path}")
    scenarios = large_library_scenarios(repo_root)
    result: list[Corpus] = []
    repositories: set[str] = set()
    for raw in manifest.get("corpora", []):
        corpus = corpus_from_manifest(raw)
        if corpus.language not in languages:
            continue
        if corpus.repository in repositories:
            raise ValueError(f"duplicate large-library corpus: {corpus.repository}")
        repositories.add(corpus.repository)
        scenario = scenarios.get(corpus.scenario_id)
        if scenario is None:
            raise ValueError(
                f"large-library corpus {corpus.repository} lacks scenario {corpus.scenario_id}"
            )
        validate_corpus_scenario(corpus, scenario)
        result.append(corpus)
    if not result:
        raise ValueError("large-library runtime benchmark selected no corpora")
    return sorted(result, key=lambda corpus: (corpus.language, corpus.repository))


def large_library_scenarios(repo_root: Path) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for path in discover_scenarios(repo_root, []):
        scenario = load_scenario(path, repo_root)
        scenario_id = scenario.get("id")
        if isinstance(scenario_id, str) and "large-library" in string_list(
            scenario.get("coverage")
        ):
            result[scenario_id] = scenario
    return result
