# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Assertions for the cross-language workspace/search contract gate."""

from __future__ import annotations

from .language_workspace_search_contract_cases import SearchContractCase
from .language_workspace_search_contract_types import AspResult, ContractFailure


def assert_workspace_result(result: AspResult, case: SearchContractCase) -> None:
    label = f"{case.language} workspace"
    if case.language == "julia":
        _assert_julia_workspace(result, case, label)
        return
    if case.workspace_router_next_prime and _looks_like_workspace_router_next_prime(result.stdout):
        _assert_workspace_router_next_prime(result.stdout, label)
        return
    _assert_workspace_graph(result.stdout, label)
    _assert_case_needles(result.stdout, case.workspace_needles, label)


def assert_ingest_result(result: AspResult, case: SearchContractCase) -> None:
    label = f"{case.language} ingest"
    _assert_ingest_graph(result.stdout, label)
    if case.language == "julia" and "legend:" not in result.stdout:
        _assert_julia_ingest_fallback(result.stdout, label)


def assert_registry(value: str, case: SearchContractCase) -> None:
    label = f"{case.language} registry"
    _assert_contains(value, '"method":"search/workspace"', label)
    _assert_contains(value, case.accepted_pipes_json, label)


def assert_failure_contains(result: AspResult, label: str, needle: str) -> None:
    if result.returncode == 0:
        raise ContractFailure(
            f"{label}: expected command to fail\n"
            f"args: {' '.join(result.args)}\n"
            f"{result.combined_output}"
        )
    _assert_contains(result.combined_output, needle, label)


def expect_success(result: AspResult, label: str) -> AspResult:
    if result.returncode != 0:
        raise ContractFailure(
            f"{label}: expected command to succeed\n"
            f"args: {' '.join(result.args)}\n"
            f"{result.combined_output}"
        )
    return result


def _assert_julia_workspace(
    result: AspResult,
    case: SearchContractCase,
    label: str,
) -> None:
    _assert_contains(result.stdout, "[search-workspace]", label)
    _assert_not_contains(
        result.combined_output,
        "expected at most one PROJECT_ROOT argument",
        label,
    )
    if "legend:" in result.stdout:
        _assert_workspace_graph(result.stdout, label)
        _assert_case_needles(result.stdout, case.workspace_needles, label)
        return
    _assert_contains(result.stdout, "scope=workspace", label)
    _assert_contains(result.stdout, "|seed owner:", label)


def _assert_workspace_graph(value: str, label: str) -> None:
    _assert_contains(value, "[search-workspace]", label)
    _assert_contains(value, "legend:", label)
    _assert_contains(value, "frontier=", label)
    _assert_not_contains(value, "G>{}", label)
    _assert_not_contains(value, "frontier=\n", label)
    _assert_not_contains(value, "expected at most one PROJECT_ROOT argument", label)


def _assert_workspace_router_next_prime(value: str, label: str) -> None:
    _assert_contains(value, "[search-workspace]", label)
    _assert_contains(value, "legend:", label)
    _assert_contains(value, "aliases: graph:{G=search}", label)
    _assert_contains(value, "G>{}", label)
    _assert_contains(value, "|next prime:.", label)
    _assert_not_contains(value, "expected at most one PROJECT_ROOT argument", label)


def _looks_like_workspace_router_next_prime(value: str) -> bool:
    return "G>{}" in value or "|next prime:." in value


def _assert_ingest_graph(value: str, label: str) -> None:
    _assert_contains(value, "[search-ingest]", label)
    _assert_not_contains(value, "expected at most one PROJECT_ROOT argument", label)


def _assert_julia_ingest_fallback(value: str, label: str) -> None:
    if "pipes=owner,tests" in value or "stdin-required" in value:
        return
    raise ContractFailure(
        f"{label}: expected output to contain 'pipes=owner,tests' or 'stdin-required'\n{value}"
    )


def _assert_case_needles(
    value: str,
    needles: tuple[str, ...],
    label: str,
) -> None:
    for needle in needles:
        _assert_contains(value, needle, label)


def _assert_contains(value: str, needle: str, label: str) -> None:
    if needle not in value:
        raise ContractFailure(f"{label}: expected output to contain {needle!r}\n{value}")


def _assert_not_contains(value: str, needle: str, label: str) -> None:
    if needle in value:
        raise ContractFailure(f"{label}: expected output to omit {needle!r}\n{value}")
