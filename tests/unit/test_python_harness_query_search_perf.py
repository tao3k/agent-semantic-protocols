"""Performance guards for ASP Python query/search warm paths."""

from __future__ import annotations

from pathlib import Path

from tests.unit._python_harness_query_search_perf_support import (
    SEARCH_TERM,
    SEARCH_PREFLIGHT_BUDGET_MS,
    SINGLE_FILE_RG_BUDGET_MS,
    WARM_ASP_BUDGET_MS,
    lexical_command,
    python_semantic_language_descriptors,
    require_release_asp,
    run_asp_python,
    search_commands_for_descriptor,
    timed_asp_python,
    timed_asp,
    write_python_fixture,
)


def test_release_search_cli_millisecond_gate() -> None:
    require_release_asp()
    broad_query = [
        "rust",
        "search",
        "pipe",
        "search performance provider startup preflight",
        "--workspace",
        "crates/agent-semantic-search",
        "--view",
        "seeds",
    ]
    single_file_rg = [
        "rg",
        "--query",
        "search_terms_budget_block",
        "--workspace",
        "crates/agent-semantic-search/src/search_query_budget.rs",
    ]

    timed_asp(broad_query)
    timed_asp(single_file_rg)
    preflight_ms, preflight_output = timed_asp(broad_query)
    rg_ms, rg_output = timed_asp(single_file_rg)

    assert "source=blocked ranker=query-budget" in preflight_output
    assert "search_query_budget.rs" in rg_output
    assert preflight_ms < SEARCH_PREFLIGHT_BUDGET_MS
    assert rg_ms < SINGLE_FILE_RG_BUDGET_MS


def test_asp_python_search_and_query_warm_paths_are_millisecond_scale(
    tmp_path: Path,
) -> None:
    project = write_python_fixture(tmp_path)
    query_command = [
        "query",
        "src/example.py",
        "--term",
        SEARCH_TERM,
        "--workspace",
        str(project),
        "--names-only",
    ]

    run_asp_python(lexical_command(project), project)
    search_ms, search_out = timed_asp_python(lexical_command(project), project)
    run_asp_python(query_command, project)
    query_ms, query_out = timed_asp_python(query_command, project)

    assert SEARCH_TERM in search_out
    assert SEARCH_TERM in query_out
    assert search_ms < WARM_ASP_BUDGET_MS
    assert query_ms < WARM_ASP_BUDGET_MS


def test_python_harness_registered_search_views_are_fast_on_warm_path(
    tmp_path: Path,
) -> None:
    project = write_python_fixture(tmp_path)
    descriptors = [
        descriptor
        for descriptor in python_semantic_language_descriptors(project)
        if str(descriptor["method"]).startswith("search/")
    ]
    registered_methods = {str(descriptor["method"]) for descriptor in descriptors}

    assert "search/lexical" in registered_methods
    assert "search/lexical" not in registered_methods
    assert _covered_search_methods(descriptors, project) == registered_methods


def _covered_search_methods(
    descriptors: list[dict[str, object]],
    project: Path,
) -> set[str]:
    covered_methods: set[str] = set()
    for descriptor in descriptors:
        for command in search_commands_for_descriptor(descriptor, project):
            run_asp_python(command.args, project, stdin=command.stdin)
            elapsed_ms, output = timed_asp_python(
                command.args,
                project,
                stdin=command.stdin,
            )

            assert output.strip(), command.args
            assert elapsed_ms < WARM_ASP_BUDGET_MS, command.args
            covered_methods.add(str(descriptor["method"]))
    return covered_methods
