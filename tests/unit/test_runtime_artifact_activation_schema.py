import json
from pathlib import Path


SCHEMA_PATH = Path("schemas/runtime-artifact-activation.schema.json")


def test_activation_generation_is_a_required_positive_publication_fence() -> None:
    schema = json.loads(SCHEMA_PATH.read_text())

    assert "activationGeneration" in schema["required"]
    assert schema["properties"]["activationGeneration"] == {
        "type": "integer",
        "minimum": 1,
    }

