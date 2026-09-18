# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shape contracts for the reusable Project Topology library."""

from copy import deepcopy
import json
from pathlib import Path

import pytest
from jsonschema.exceptions import ValidationError

from unit.schema_validation import schema_validator_for

ROOT = Path(__file__).parents[2]
SCHEMA_PATH = ROOT / "schemas/project-topology-library.v1.schema.json"
VALID = json.loads(
    (
        ROOT / "schemas/fixtures/project-topology-library/valid-polyglot.v1.json"
    ).read_text()
)
VALIDATOR = schema_validator_for(SCHEMA_PATH)


def project_workspace_binding():
    return json.loads(
        (
            ROOT
            / "schemas/fixtures/project-workspace-binding/valid-cross-machine.v1.json"
        ).read_text()
    )


def test_polyglot_project_topology_is_shared_across_non_search_consumers():
    VALIDATOR.validate(VALID)
    assert {
        "search",
        "query",
        "code-understanding",
        "framework-calibration",
        "refactoring",
        "context-recovery",
    } == set(VALID["consumers"])


def test_v1_requires_expected_relations_and_frontiers() -> None:
    assert "expectedRelations" in VALIDATOR.schema["required"]
    assert "frontiers" in VALIDATOR.schema["required"]


@pytest.mark.parametrize("field", ["target", "relation", "coverage"])
def test_expected_relation_shape_is_closed(field) -> None:
    expected_schema = VALIDATOR.schema["$defs"]["expectedRelation"]
    assert expected_schema["additionalProperties"] is False
    assert field in expected_schema["required"]


def test_topology_uses_the_shared_project_workspace_binding():
    packet = deepcopy(VALID)
    packet["projectWorkspace"] = project_workspace_binding()
    VALIDATOR.validate(packet)
    assert packet["projectWorkspace"]["workspaceRootPath"] == "."


def test_parser_owner_node_uses_a_non_query_owner_locator():
    packet = deepcopy(VALID)
    packet["nodes"][0].pop("selector")
    packet["nodes"][0]["ownerLocator"] = "rust://src/registry.rs"
    packet["nodes"][0]["kind"] = "Owner"
    VALIDATOR.validate(packet)


def test_owner_locator_cannot_impersonate_an_exact_item_selector():
    packet = deepcopy(VALID)
    packet["nodes"][0]["ownerLocator"] = "rust://src/registry.rs"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


@pytest.mark.parametrize("field", ["rank", "hit", "match", "read"])
def test_library_rejects_request_local_retrieval_projection(field):
    packet = deepcopy(VALID)
    packet["nodes"][0][field] = 1
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_library_rejects_partial_generation_publication():
    packet = deepcopy(VALID)
    packet["generation"]["state"] = "partial"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_library_requires_a_typed_from_scratch_rebuild_receipt():
    packet = deepcopy(VALID)
    packet.pop("fromScratchRebuildReceipt")
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_derived_closure_requires_typed_proof_dependencies():
    packet = deepcopy(VALID)
    packet["closure"].pop("proofDependencies")
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_accepted_semantic_annotation_requires_admission_receipt():
    packet = deepcopy(VALID)
    packet["nodes"][3]["annotation"]["state"] = "accepted"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_derived_edge_requires_proof_reference():
    packet = deepcopy(VALID)
    packet["edges"][0]["modality"] = "derived"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_library_has_exactly_one_terminal():
    packet = deepcopy(VALID)
    packet["terminal"]["terminalCount"] = 2
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)
