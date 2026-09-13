# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Contract tests for the MRR-defined Project Topology program binding."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

import pytest

from asp_proofs.project_topology_program import (
    TopologyProgramError,
    validate_project_topology_program,
)


ROOT = Path(__file__).resolve().parents[3]
SCHEMA = json.loads(
    (ROOT / "schemas/project-topology-program-binding.v1.schema.json").read_text()
)
PROJECT_WORKSPACE_SCHEMA = json.loads(
    (ROOT / "schemas/project-workspace-binding.v1.schema.json").read_text()
)
VALID = ROOT / "schemas/fixtures/project-topology-program-binding/valid-project.v1.json"


@pytest.fixture()
def manifest():
    return json.loads(VALID.read_text())


def admitted_receipts(manifest):
    receipt = manifest["program"]["compilationReceipt"]
    return {receipt["id"]: deepcopy(receipt)}


def validate(manifest, receipts):
    validate_project_topology_program(
        manifest, SCHEMA, receipts, [PROJECT_WORKSPACE_SCHEMA]
    )


def test_valid_program_uses_mrr_directly_without_an_extension_catalog(manifest):
    validate(manifest, admitted_receipts(manifest))
    assert "extensionCatalog" not in manifest
    assert manifest["binding"]["projectWorkspace"]["projectWorkspaceIdentity"].startswith(
        "git+https://"
    )
    assert manifest["binding"]["mrrBundleIdentity"].startswith(
        "mrr:reasoning-bundle:v1:"
    )


def test_content_digest_cannot_impersonate_the_mrr_bundle_identity(manifest):
    manifest["binding"]["mrrBundleIdentity"] = manifest["binding"][
        "schemeProgramDigest"
    ]
    with pytest.raises(TopologyProgramError) as caught:
        validate(manifest, admitted_receipts(manifest))
    assert caught.value.reason_kind == "topology-program-schema-invalid"


def test_runtime_generated_workspace_id_cannot_replace_gitops_project_identity(manifest):
    manifest["binding"]["projectWorkspace"]["projectWorkspaceIdentity"] = (
        "workspace-example"
    )
    with pytest.raises(TopologyProgramError) as caught:
        validate(manifest, admitted_receipts(manifest))
    assert caught.value.reason_kind == "topology-program-schema-invalid"


def test_all_six_standard_topology_profiles_are_required(manifest):
    manifest["profiles"].remove("mrr.topology.reference.v1")
    with pytest.raises(TopologyProgramError) as caught:
        validate(manifest, admitted_receipts(manifest))
    assert caught.value.reason_kind == "topology-standard-profile-missing"


def test_project_program_cannot_replace_a_standard_namespace(manifest):
    module = manifest["program"]["modules"][0]
    module.update(
        {
            "operation": "replace",
            "ownerNamespace": "mrr.topology",
        }
    )
    with pytest.raises(TopologyProgramError) as caught:
        validate(manifest, admitted_receipts(manifest))
    assert caught.value.reason_kind == "topology-program-replacement-owner-mismatch"


def test_compilation_receipt_must_be_independently_admitted_and_exact(manifest):
    admitted = admitted_receipts(manifest)
    manifest["program"]["compilationReceipt"]["compiledProgramAbiDigest"] = (
        "blake3-256:" + "f" * 64
    )
    with pytest.raises(TopologyProgramError) as caught:
        validate(manifest, admitted)
    assert caught.value.reason_kind == "topology-program-compilation-receipt-mismatch"


def test_topology_materialization_must_exclude_itself_from_inputs(manifest):
    manifest["materialization"]["excludedInputs"].remove(
        ".agents/asp/topology/**"
    )
    with pytest.raises(TopologyProgramError) as caught:
        validate(manifest, admitted_receipts(manifest))
    assert caught.value.reason_kind == "topology-self-indexing-not-excluded"


def test_parallel_extension_catalog_is_rejected_by_the_closed_shape(manifest):
    manifest["extensionCatalog"] = {"digest": "blake3-256:" + "a" * 64}
    with pytest.raises(TopologyProgramError) as caught:
        validate(manifest, admitted_receipts(manifest))
    assert caught.value.reason_kind == "topology-program-schema-invalid"


def test_admitted_terminal_cannot_carry_a_failure_reason(manifest):
    manifest["terminal"]["reasonKind"] = "topology-program-compilation-failed"
    with pytest.raises(TopologyProgramError) as caught:
        validate(manifest, admitted_receipts(manifest))
    assert caught.value.reason_kind == "topology-program-schema-invalid"
