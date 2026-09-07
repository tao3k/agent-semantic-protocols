# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError
import pytest
from referencing import Registry, Resource

from asp_proofs.query_playbook_materialization import validate_query_playbook_request
from asp_proofs.query_playbook_materialization import validate_query_playbook_receipt
from asp_proofs.search_topology_settlement import SettlementError


ROOT = Path(__file__).resolve().parents[3]
SCHEMA_DIR = ROOT / "schemas"
REQUEST_PATH = SCHEMA_DIR / (
    "fixtures/query-playbook-materialization-request/valid-runtime-bound.v1.json"
)
RECEIPT_PATH = SCHEMA_DIR / (
    "fixtures/query-playbook-materialization-receipt/valid-runtime-bound-ready.v1.json"
)


def load(path: Path) -> dict:
    return json.loads(path.read_text())


def manifest_project_workspace(request: dict) -> dict:
    return deepcopy(request["runtimeExecutionBinding"]["projectWorkspace"])


def request_validator() -> Draft202012Validator:
    schema_paths = [
        SCHEMA_DIR / "query-playbook-materialization-request.v1.schema.json",
        SCHEMA_DIR / "project-workspace-binding.v1.schema.json",
        SCHEMA_DIR / "runtime-execution-binding.v2.schema.json",
        SCHEMA_DIR / "content-binding.schema.json",
    ]
    schemas = [load(path) for path in schema_paths]
    registry = Registry().with_resources(
        [(schema["$id"], Resource.from_contents(schema)) for schema in schemas]
    )
    return Draft202012Validator(schemas[0], registry=registry)


def receipt_validator() -> Draft202012Validator:
    schema_paths = [
        SCHEMA_DIR / "query-playbook-materialization-receipt.v1.schema.json",
        SCHEMA_DIR / "project-workspace-binding.v1.schema.json",
        SCHEMA_DIR / "runtime-execution-binding.v2.schema.json",
        SCHEMA_DIR / "content-binding.schema.json",
    ]
    schemas = [load(path) for path in schema_paths]
    registry = Registry().with_resources(
        [(schema["$id"], Resource.from_contents(schema)) for schema in schemas]
    )
    return Draft202012Validator(schemas[0], registry=registry)


def test_runtime_bound_fixture_is_schema_valid_and_semantically_admitted() -> None:
    request = load(REQUEST_PATH)
    request_validator().validate(request)
    validate_query_playbook_request(
        request,
        request["runtimeExecutionBinding"],
        manifest_project_workspace(request),
    )


def test_query_v1_rejects_the_legacy_runtime_identity_carrier() -> None:
    request = load(REQUEST_PATH)
    binding = request["runtimeExecutionBinding"]
    binding["schemaId"] = "asp.runtime-execution-binding"
    binding["schemaVersion"] = "1"
    binding["projectId"] = binding.pop("projectWorkspace")[
        "projectWorkspaceIdentity"
    ]
    binding["workspaceId"] = binding.pop("worktreeInstanceId")
    with pytest.raises(ValidationError):
        request_validator().validate(request)


def test_query_accepts_the_smallest_selector_subset_without_search_handoff() -> None:
    request = load(REQUEST_PATH)
    request["selectors"] = request["selectors"][:1]
    request_validator().validate(request)
    validate_query_playbook_request(
        request,
        request["runtimeExecutionBinding"],
        manifest_project_workspace(request),
    )


def test_query_rejects_runtime_binding_drift() -> None:
    request = load(REQUEST_PATH)
    admitted = deepcopy(request["runtimeExecutionBinding"])
    request["runtimeExecutionBinding"]["evaluatorPolicyDigest"] = (
        "blake3-256:" + "a" * 64
    )
    with pytest.raises(SettlementError) as caught:
        validate_query_playbook_request(
            request, admitted, manifest_project_workspace(request)
        )
    assert caught.value.reason_kind == "query-playbook-runtime-binding-mismatch"


@pytest.mark.parametrize("field", ["projectWorkspaceIdentity", "worktreeInstanceId"])
def test_query_rejects_project_or_worktree_context_drift(field: str) -> None:
    request = load(REQUEST_PATH)
    request[field] = "foreign-context"
    with pytest.raises(SettlementError) as caught:
        validate_query_playbook_request(
            request,
            request["runtimeExecutionBinding"],
            manifest_project_workspace(request),
        )
    assert caught.value.reason_kind == "query-playbook-runtime-context-mismatch"


def test_equal_query_and_runtime_cannot_self_authorize_a_foreign_workspace() -> None:
    request = load(REQUEST_PATH)
    manifest = manifest_project_workspace(request)
    foreign = deepcopy(manifest)
    foreign["projectWorkspaceIdentity"] = (
        "git+https://github.com/tao3k/foreign.git#workspace/root"
    )
    request["projectWorkspaceIdentity"] = foreign["projectWorkspaceIdentity"]
    request["runtimeExecutionBinding"]["projectWorkspace"] = foreign

    with pytest.raises(SettlementError) as caught:
        validate_query_playbook_request(
            request, request["runtimeExecutionBinding"], manifest
        )
    assert (
        caught.value.reason_kind
        == "query-playbook-project-workspace-manifest-mismatch"
    )


def test_legacy_search_settlement_fields_are_not_part_of_query_v1() -> None:
    request = load(REQUEST_PATH)
    request["settlementBinding"] = {}
    with pytest.raises(ValidationError):
        request_validator().validate(request)


def test_query_rejects_search_only_matches_projection() -> None:
    request = load(REQUEST_PATH)
    request["projection"] = "matches"
    with pytest.raises(ValidationError):
        request_validator().validate(request)


def test_ready_receipt_is_one_runtime_bound_all_or_nothing_terminal() -> None:
    request = load(REQUEST_PATH)
    receipt = load(RECEIPT_PATH)
    receipt_validator().validate(receipt)
    validate_query_playbook_receipt(
        receipt,
        request,
        request["runtimeExecutionBinding"],
        manifest_project_workspace(request),
    )


def test_ready_receipt_rejects_a_missing_or_reordered_selector() -> None:
    request = load(REQUEST_PATH)
    receipt = load(RECEIPT_PATH)
    receipt["materializations"].reverse()
    with pytest.raises(SettlementError) as caught:
        validate_query_playbook_receipt(
            receipt,
            request,
            request["runtimeExecutionBinding"],
            manifest_project_workspace(request),
        )
    assert caught.value.reason_kind == "query-playbook-materialization-set-mismatch"


def test_failed_receipt_cannot_expose_partial_materialization() -> None:
    receipt = load(RECEIPT_PATH)
    receipt["terminal"] = {
        "state": "failed",
        "terminalCount": 1,
        "reasonKind": "selector-stale",
    }
    with pytest.raises(ValidationError):
        receipt_validator().validate(receipt)
