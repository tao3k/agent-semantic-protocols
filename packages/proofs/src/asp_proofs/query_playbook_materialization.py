# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Runtime-bound semantic admission for Query Playbook materialization."""

from __future__ import annotations

from ._topology_admission import require_topology


def validate_query_playbook_request(
    request: dict,
    expected_runtime_binding: dict,
    expected_execution_publication_digest: str,
    expected_runtime_bundle_digest: str,
) -> None:
    """Require exact Runtime identity and preserve the caller's selector order."""

    require_topology(
        request["runtimeExecutionBinding"] == expected_runtime_binding,
        "query-playbook-runtime-binding-mismatch",
    )
    require_topology(
        request["runtimeWorkspaceExecutionPublicationDigest"]
        == expected_execution_publication_digest
        and request["runtimeBundleDigest"] == expected_runtime_bundle_digest,
        "query-playbook-execution-publication-mismatch",
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
        and len(selectors) == len(set(selectors))
        and all("://" in selector and "#item/" in selector for selector in selectors),
        "schema-invalid",
    )


def validate_query_playbook_receipt(
    receipt: dict,
    request: dict,
    expected_runtime_binding: dict,
    expected_execution_publication_digest: str,
    expected_runtime_bundle_digest: str,
) -> None:
    """Require one binding, caller-ordered results, and one terminal."""

    require_topology(
        receipt["runtimeExecutionBinding"] == expected_runtime_binding,
        "query-playbook-runtime-binding-mismatch",
    )
    require_topology(
        receipt["runtimeWorkspaceExecutionPublicationDigest"]
        == expected_execution_publication_digest
        and receipt["runtimeBundleDigest"] == expected_runtime_bundle_digest,
        "query-playbook-execution-publication-mismatch",
    )
    require_topology(
        receipt["requestId"] == request["requestId"]
        and receipt["projectWorkspaceIdentity"] == request["projectWorkspaceIdentity"]
        and receipt["worktreeInstanceId"] == request["worktreeInstanceId"]
        and receipt["projection"] == request["projection"]
        and receipt["requestedSelectors"] == request["selectors"]
        and receipt["runtimeWorkspaceExecutionPublicationDigest"]
        == request["runtimeWorkspaceExecutionPublicationDigest"]
        and receipt["runtimeBundleDigest"] == request["runtimeBundleDigest"],
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
            and all(
                item["projection"] == request["projection"] for item in materializations
            ),
            "query-playbook-materialization-set-mismatch",
        )
    else:
        require_topology(
            not materializations,
            "query-playbook-failure-exposed-partial-materialization",
        )
