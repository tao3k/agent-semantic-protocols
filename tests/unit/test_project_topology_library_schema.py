"""Shape contracts for the reusable Project Topology library."""

from copy import deepcopy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError

ROOT = Path(__file__).parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/project-topology-library.v1.schema.json").read_text()
)
VALID = json.loads(
    (
        ROOT / "schemas/fixtures/project-topology-library/valid-polyglot.v1.json"
    ).read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


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
