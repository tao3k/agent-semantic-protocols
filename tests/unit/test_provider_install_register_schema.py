import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def load(path: str) -> dict:
    return json.loads((ROOT / path).read_text())


def test_provider_install_register_is_schema_valid_and_complete() -> None:
    schema = load("schemas/provider-install-register.schema.json")
    register = load("schemas/provider-install-register.json")
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(register)

    providers = register["providers"]
    assert {provider["languageId"] for provider in providers} == {
        "rust",
        "typescript",
        "python",
        "julia",
        "gerbil-scheme",
    }
    assert len({provider["providerId"] for provider in providers}) == len(providers)
    for provider in providers:
        expected = f"asp-{provider['languageId']}"
        assert provider["providerId"] == expected
        assert provider["binary"] == expected


def test_provider_install_register_has_no_hook_or_cli_authority() -> None:
    register = load("schemas/provider-install-register.json")
    serialized = json.dumps(register, sort_keys=True)
    for forbidden in (
        "languageProviders",
        "manifestDigest",
        "methodDescriptor",
        "runtimeProfile",
        "rs-harness",
    ):
        assert forbidden not in serialized
