import copy
import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[3]
SCHEMA = json.loads((ROOT / "schemas/semantic-how-frame.v1.schema.json").read_text())


def valid_how_frame():
    return {
        "schemaId": "semantic-how-frame.v1",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols",
        "protocolVersion": "1",
        "frameId": "how-1",
        "whereFrameId": "where-1",
        "topologyId": "topology-1",
        "decision": {"changeShape": "search-then-edit"},
        "searchActionFrame": {
            "frameId": "search-1",
            "actionKind": "search",
            "intent": "locate source-index owner evidence",
            "command": "asp rust search owner src/lib.rs items --query fixture --workspace . --view seeds",
            "evidence": [{"id": "ev-search", "kind": "source-index"}],
            "stopCondition": "owner selector evidence found",
            "avoid": ["line-range-selector"],
        },
        "editActionFrame": {
            "frameId": "edit-1",
            "actionKind": "edit",
            "mutationKind": "apply-patch",
            "target": "src/lib.rs",
            "preconditions": ["searchActionFrame evidence is present"],
            "validation": [{"kind": "test", "commandOrId": "cargo test focused"}],
        },
        "why": [{"id": "fact-1", "summary": "search and edit have different contracts"}],
        "illegal": [{"branch": "line-range-query", "reason": "display range is not identity"}],
        "validate": [{"kind": "schema", "commandOrId": "semantic-how-frame.v1"}],
        "evidence": [{"id": "ev-search", "kind": "source-index"}],
        "branchLegality": {
            "targetRole": "writer",
            "trustBoundary": "internal-contract",
            "allowedRecoveries": ["route-to-owner"],
            "prunedBranches": [],
            "evidenceGaps": [],
            "validation": ["schema", "test"],
        },
    }


def test_how_frame_accepts_separate_search_and_edit_action_frames():
    jsonschema.Draft202012Validator(SCHEMA).validate(valid_how_frame())


def test_search_action_frame_cannot_use_edit_action_kind():
    frame = valid_how_frame()
    frame["searchActionFrame"]["actionKind"] = "edit"

    validator = jsonschema.Draft202012Validator(SCHEMA)
    errors = list(validator.iter_errors(frame))

    assert any("search" in str(error.message) for error in errors)


def test_edit_action_frame_requires_mutation_kind():
    frame = valid_how_frame()
    edit_frame = copy.deepcopy(frame["editActionFrame"])
    del edit_frame["mutationKind"]
    frame["editActionFrame"] = edit_frame

    validator = jsonschema.Draft202012Validator(SCHEMA)
    errors = list(validator.iter_errors(frame))

    assert any("mutationKind" in str(error.message) for error in errors)
