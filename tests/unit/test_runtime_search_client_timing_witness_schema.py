import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/runtime-search-client-timing-witness.v1.schema.json").read_text()
)


def witness():
    return {
        "schemaId": "agent.semantic-protocols.runtime-search-client-timing-witness",
        "schemaVersion": "1",
        "sessionId": "session-1",
        "requestId": "request-1",
        "phases": [
            {"name": "launcher", "elapsedMicros": 1},
            {"name": "client-frame-encode", "elapsedMicros": 2},
            {"name": "ipc-connect", "elapsedMicros": 3},
        ],
    }


def test_client_timing_witness_is_ordered_and_identity_free():
    jsonschema.Draft202012Validator(SCHEMA).validate(witness())
    forbidden = {
        "workspaceIdentity",
        "runtimeArtifactDigest",
        "runtimeBundleDigest",
        "executionPublicationDigest",
        "sourceGenerationDigest",
        "sourceIndexDigest",
        "providerCatalogDigest",
        "activationGeneration",
    }
    assert forbidden.isdisjoint(SCHEMA["properties"])


def test_client_timing_witness_rejects_reordered_phases():
    candidate = witness()
    candidate["phases"].reverse()
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(candidate)


@pytest.mark.parametrize("field", ["next", "nextAction", "recommendedNext", "plannerDecision"])
def test_client_timing_witness_rejects_agent_planner_fields(field):
    candidate = witness()
    candidate[field] = "query"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(candidate)
