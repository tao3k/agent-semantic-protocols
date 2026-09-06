import json
from pathlib import Path

from jsonschema import Draft202012Validator


SCHEMA_PATH = Path("schemas/runtime-artifact-bundle-binding.v1.schema.json")
BUNDLE_SCHEMA_PATH = Path("schemas/runtime-binary-bundle.schema.json")


def load_schema() -> dict:
    return json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))


def test_runtime_artifact_bundle_binding_is_a_closed_v1_contract() -> None:
    schema = load_schema()
    Draft202012Validator.check_schema(schema)

    assert schema["properties"]["schemaId"]["const"] == (
        "agent.semantic-protocols.runtime-artifact-bundle-binding"
    )
    assert schema["properties"]["schemaVersion"]["const"] == "1"
    assert schema["additionalProperties"] is False
    assert set(schema["required"]) == {
        "schemaId",
        "schemaVersion",
        "providerRegistrationDigest",
        "installedProviderBindingDigest",
        "evaluatorPolicyDigest",
        "schemaBundleDigest",
    }


def test_bundle_binding_accepts_only_blake3_content_identities() -> None:
    digest = load_schema()["$defs"]["blake3Digest"]

    assert digest == {
        "type": "string",
        "pattern": "^blake3-256:[0-9a-f]{64}$",
    }


def test_publication_sequence_is_not_product_identity() -> None:
    serialized = json.dumps(load_schema(), sort_keys=True)

    assert "activationGeneration" not in serialized
    assert "publicationSequence" not in serialized


def test_runtime_binary_bundle_requires_the_materialized_execution_closure() -> None:
    schema = json.loads(BUNDLE_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)

    assert schema["properties"]["schemaId"]["const"] == (
        "agent.semantic-protocols.runtime-binary-bundle"
    )
    assert schema["properties"]["schemaVersion"]["const"] == 1
    assert schema["additionalProperties"] is False
    assert "executionBinding" in schema["required"]
    assert set(schema["properties"]["members"]["required"]) == {
        "provider-registration.json",
        "installed-provider-binding.json",
        "evaluator-policy.json",
        "schema-bundle.json",
    }
