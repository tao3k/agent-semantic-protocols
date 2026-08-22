import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def load(path: str) -> dict[str, object]:
    return json.loads((ROOT / path).read_text())


def test_provider_register_document_is_schema_owned() -> None:
    schema = load("schemas/provider-register.schema.json")
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(load("schemas/provider-register.json"))


def test_external_provider_register_request_uses_the_shared_wire_schema() -> None:
    schema = load("schemas/provider-register-request.schema.json")
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(
        {
            "schemaId": "agent.semantic-protocols.provider-register.request",
            "schemaVersion": "1",
            "expectedGeneration": 4,
            "request": {
                "operation": "register",
                "provider": {
                    "languageId": "zig",
                    "providerId": "asp-zig",
                    "registration": {
                        "languageId": "zig",
                        "providerId": "asp-zig",
                    },
                },
            },
        }
    )


def test_generation_conflict_is_a_typed_provider_register_response() -> None:
    schema = load("schemas/provider-register-response.schema.json")
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(
        {
            "schemaId": "agent.semantic-protocols.provider-register.response",
            "schemaVersion": "1",
            "result": {
                "outcome": "generation-conflict",
                "actualGeneration": 9,
            },
        }
    )


def test_external_provider_state_has_a_dedicated_schema() -> None:
    schema = load("schemas/provider-register-state.schema.json")
    Draft202012Validator.check_schema(schema)
