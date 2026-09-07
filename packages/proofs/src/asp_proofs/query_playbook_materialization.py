# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Runtime-bound semantic admission for Query Playbook materialization."""

from __future__ import annotations

from ._topology_admission import require_topology


def validate_query_playbook_request(
    request: dict,
    expected_runtime_binding: dict,
    manifest_project_workspace: dict,
) -> None:
    """Require exact Runtime identity while keeping Query independent of Search."""

    require_topology(
        expected_runtime_binding.get("projectWorkspace")
        == manifest_project_workspace,
        "query-playbook-project-workspace-manifest-mismatch",
    )
    require_topology(
        request["runtimeExecutionBinding"] == expected_runtime_binding,
        "query-playbook-runtime-binding-mismatch",
    )
    require_topology(
        request["projectWorkspaceIdentity"]
        == expected_runtime_binding.get("projectWorkspace", {}).get(
            "projectWorkspaceIdentity"
        )
        and request["worktreeInstanceId"]
        == expected_runtime_binding.get("worktreeInstanceId"),
        "query-playbook-runtime-context-mismatch",
    )
    selectors = request["selectors"]
    require_topology(
        bool(selectors)
        and selectors == sorted(set(selectors))
        and all("://" in selector and "#item/" in selector for selector in selectors),
        "schema-invalid",
    )


def validate_query_playbook_receipt(
    receipt: dict,
    request: dict,
    expected_runtime_binding: dict,
    manifest_project_workspace: dict,
) -> None:
    """Require one binding, all requested materializations, and one terminal."""

    require_topology(
        expected_runtime_binding.get("projectWorkspace")
        == manifest_project_workspace,
        "query-playbook-project-workspace-manifest-mismatch",
    )
    require_topology(
        receipt["runtimeExecutionBinding"] == expected_runtime_binding,
        "query-playbook-runtime-binding-mismatch",
    )
    require_topology(
        receipt["requestId"] == request["requestId"]
        and receipt["projectWorkspaceIdentity"]
        == request["projectWorkspaceIdentity"]
        and receipt["worktreeInstanceId"] == request["worktreeInstanceId"]
        and receipt["projection"] == request["projection"]
        and receipt["requestedSelectors"] == request["selectors"],
        "query-playbook-request-binding-mismatch",
    )
    terminal = receipt["terminal"]
    require_topology(
        terminal["terminalCount"] == 1,
        "query-playbook-terminal-count-mismatch",
    )
    materializations = receipt["materializations"]
    if terminal["state"] == "ready":
        require_topology(
            [item["selector"] for item in materializations] == request["selectors"]
            and all(item["projection"] == request["projection"] for item in materializations),
            "query-playbook-materialization-set-mismatch",
        )
    else:
        require_topology(
            not materializations,
            "query-playbook-failure-exposed-partial-materialization",
        )
