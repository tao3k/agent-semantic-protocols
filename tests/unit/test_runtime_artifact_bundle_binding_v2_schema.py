import json
from pathlib import Path

from jsonschema import Draft202012Validator


SCHEMAS = Path("schemas")


def load(name: str) -> dict:
    return json.loads((SCHEMAS / name).read_text(encoding="utf-8"))


def test_runtime_artifact_binding_v2_names_evaluator_abi_independently() -> None:
    schema = load("runtime-artifact-bundle-binding.v2.schema.json")
    Draft202012Validator.check_schema(schema)

    assert schema["properties"]["schemaVersion"]["const"] == "2"
    assert "evaluatorAbiDigest" in schema["required"]
    assert schema["additionalProperties"] is False


def test_runtime_binary_bundle_v2_materializes_evaluator_abi() -> None:
    schema = load("runtime-binary-bundle.v2.schema.json")
    Draft202012Validator.check_schema(schema)

    assert schema["properties"]["schemaVersion"]["const"] == 2
    assert "evaluator-abi.json" in schema["properties"]["members"]["required"]
    assert schema["properties"]["executionBinding"]["$ref"] == (
        "runtime-artifact-bundle-binding.v2.schema.json"
    )


def test_artifact_bundle_v2_does_not_use_activation_generation_as_abi() -> None:
    serialized = json.dumps(
        {
            "binding": load("runtime-artifact-bundle-binding.v2.schema.json"),
            "bundle": load("runtime-binary-bundle.v2.schema.json"),
        },
        sort_keys=True,
    )
    assert "activationGeneration" not in serialized
