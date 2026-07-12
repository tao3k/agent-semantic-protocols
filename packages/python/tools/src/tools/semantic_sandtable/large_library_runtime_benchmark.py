"""Orchestrate a complete public-ASP benchmark across pinned large libraries."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
from typing import Any

from .large_library_report_chain import DEFAULT_LANGUAGES
from .large_library_runtime_deployment import (
    install_workspace_providers,
    release_binary_is_valid,
)
from .large_library_runtime_registry import search_descriptors
from .large_library_runtime_receipt import coverage, empty_coverage, runtime_receipt
from .large_library_runtime_steps import benchmark_fd_step, benchmark_step, warmup
from .large_library_runtime_types import Corpus
from .scenario_io import discover_scenarios, load_scenario
from .utils import dict_value, string_list


_CORPUS_MANIFEST = "benchmarks/large-library-runtime-corpora.v1.json"


def run_large_library_runtime_benchmark(
    repo_root: Path,
    *,
    asp_binary: Path,
    corpus_root: Path,
    languages: tuple[str, ...] = DEFAULT_LANGUAGES,
) -> dict[str, Any]:
    """Install live providers and execute every registered search method."""
    selected_languages = tuple(sorted(set(languages)))
    corpora = load_corpora(repo_root, selected_languages)
    binary = asp_binary.expanduser().resolve()
    release_verified = release_binary_is_valid(binary)
    resolved, missing = resolve_corpora(corpora, corpus_root)
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
        binary,
        corpora,
        corpus_root,
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
    corpus_root: Path,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], set[str], int]:
    steps: list[dict[str, Any]] = []
    warmups: list[dict[str, Any]] = []
    registered_methods: set[str] = set()
    command_count = 0
    for corpus in corpora:
        workspace = corpus_path(corpus, corpus_root)
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


def corpus_from_manifest(raw: Any) -> Corpus:
    record = dict_value(raw)
    inputs = dict_value(record.get("inputs"))
    values = {
        key: record.get(key)
        for key in ("scenarioId", "language", "repository", "directory", "environment")
    }
    if not all(isinstance(value, str) and value for value in values.values()):
        raise ValueError("large-library corpus manifest has incomplete identity")
    normalized_inputs = {
        key: value for key, value in inputs.items() if isinstance(value, str) and value
    }
    if set(normalized_inputs) != {"owner", "query", "dependency"}:
        raise ValueError("large-library corpus inputs must define owner, query, dependency")
    return Corpus(
        scenario_id=str(values["scenarioId"]),
        language=str(values["language"]),
        repository=str(values["repository"]),
        directory=str(values["directory"]),
        environment=str(values["environment"]),
        inputs=normalized_inputs,
    )


def validate_corpus_scenario(corpus: Corpus, scenario: dict[str, Any]) -> None:
    target = dict_value(dict_value(scenario.get("evidence")).get("targetLibrary"))
    if scenario.get("language") != corpus.language or target.get("repository") != corpus.repository:
        raise ValueError(f"large-library corpus scenario drift: {corpus.scenario_id}")


def resolve_corpora(
    corpora: list[Corpus], corpus_root: Path
) -> tuple[list[dict[str, str]], list[dict[str, str]]]:
    resolved: list[dict[str, str]] = []
    missing: list[dict[str, str]] = []
    for corpus in corpora:
        path = corpus_path(corpus, corpus_root)
        record = {
            "scenarioId": corpus.scenario_id,
            "language": corpus.language,
            "repository": corpus.repository,
            "path": str(path),
        }
        owner = path / corpus.inputs["owner"]
        if not path.is_dir():
            missing.append({**record, "reason": "checkout-missing"})
        elif not owner.is_file():
            missing.append({**record, "reason": "owner-missing"})
        else:
            resolved.append({**record, "revision": git_revision(path)})
    return resolved, missing


def corpus_path(corpus: Corpus, corpus_root: Path) -> Path:
    explicit = os.environ.get(corpus.environment)
    return (
        Path(explicit).expanduser().resolve()
        if explicit
        else (corpus_root / corpus.directory).expanduser().resolve()
    )


def git_revision(path: Path) -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=path,
        text=True,
        capture_output=True,
        check=False,
        timeout=10,
    )
    revision = completed.stdout.strip()
    return revision if completed.returncode == 0 and revision else "non-git"
