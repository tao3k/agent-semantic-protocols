# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""RFC 10.34 and schema contract tests for Marlin-native host integration."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = (
    _REPO_ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.34-marlin-host-integration-protocol.org"
)
_SCHEMA_PATH = _REPO_ROOT / "schemas" / "marlin-host-integration.v1.schema.json"


def _validator() -> Draft202012Validator:
    schema = json.loads(_SCHEMA_PATH.read_text(encoding="utf-8"))
    return Draft202012Validator(schema)


def _valid_binding() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.marlin-host-integration",
        "schemaVersion": "1",
        "platform": "marlin",
        "hostId": "marlin-local-01",
        "projectId": "proj-marlin-core",
        "workspaceId": "ws-main",
        "worktreeId": "wt-feat-marlin-native-core",
        "capabilityProfile": "full-workspace-generation",
        "admittedMethods": [
            "asp.workspace.search.playbook",
            "asp.workspace.query.playbook",
            "asp.workspace.query.syntax",
            "asp.workspace.query.syntax-plan-context",
            "asp.workspace.generation.ensure-ready",
            "asp.graph.evaluate",
            "asp.schema.bundle",
        ],
        "enginePorts": {
            "orgPort": "marlin-context-org-fold",
            "storagePort": "marlin-agent-storage-turso",
        },
        "correlation": {
            "rootSessionId": "marlin-root-01",
            "sessionId": "marlin-session-02",
            "parentSessionId": "marlin-root-01",
            "agentLineage": "marlin.root/researcher",
        },
        "stateHome": "/Users/example/.agent-semantic-protocols",
    }


def test_marlin_host_integration_rfc_documents_ownership_and_retirements() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")
    required_terms = [
        "RFC 10.34: Marlin Host Integration Protocol",
        "marlin-agent-core",
        "marlin-agent-hooks",
        "marlin-agent-sessions",
        "marlin-agent-graph",
        "marlin-agent-context",
        "marlin-agent-storage",
        "OrgDocumentProjectionPort",
        "SearchEngineStoragePort",
        "MarlinConvergedSearchEngine",
        "marlin-host-integration.v1.schema.json",
        "asp.workspace.search.playbook",
        "asp.workspace.query.playbook",
        "asp.workspace.generation.ensure-ready",
        "MARLIN_SESSION_ID",
        "MARLIN_ROOT_SESSION_ID",
        "mutationAvailable: false",
    ]
    missing = [term for term in required_terms if term not in text]
    assert missing == []


def test_retired_codex_claude_and_hook_surfaces_are_absent() -> None:
    for retired in (
        "crates/agent-semantic-hook",
        "crates/agent-semantic-hook-testkit",
        "crates/agent-semantic-shell-parser",
        "asp-codex-plugin",
        ".agents/plugins",
        "agents",
        "docs/10-19-rfcs/10.15-agent-hook-interception-protocol.org",
        "crates/agent-semantic-client-db/src/turso_agent_storage.rs",
        "crates/agent-semantic-client-db/src/turso_mvcc_store.rs",
        "crates/agent-semantic-client-db/src/turso_cdc_storage.rs",
        "crates/agent-semantic-client-db/src/turso_encrypted_storage.rs",
        "crates/agent-semantic-client-db/src/turso_sync_storage.rs",
        "crates/agent-semantic-client/src/provider_runtime_storage.rs",
    ):
        assert not (_REPO_ROOT / retired).exists(), f"retired path exists: {retired}"


def test_marlin_host_integration_schema_accepts_valid_binding() -> None:
    errors = [error.message for error in _validator().iter_errors(_valid_binding())]
    assert errors == []


def test_marlin_host_integration_schema_rejects_non_marlin_platform_and_retired_methods() -> (
    None
):
    non_marlin = _valid_binding()
    non_marlin["platform"] = "codex"
    assert list(_validator().iter_errors(non_marlin))

    retired_method = _valid_binding()
    retired_method["admittedMethods"] = ["asp.hook.memory.inbox.publish"]
    assert list(_validator().iter_errors(retired_method))
