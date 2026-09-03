from pathlib import Path

import json
import pytest
from jsonschema import Draft202012Validator, ValidationError

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]

def load(relative: str) -> dict[str, object]:
    return json.loads((ROOT / relative).read_text())


def provider_id_validator(schema_name: str) -> Draft202012Validator:
    schema = load(f"schemas/{schema_name}")
    provider_id = schema["properties"]["providerId"]
    return Draft202012Validator(provider_id)


def provider_identity_validator() -> Draft202012Validator:
    schema = load("schemas/provider-manifest.schema.json")
    identity = schema["$defs"]["canonicalProviderIdentity"]
    return Draft202012Validator(identity)


def provider_identity_branches(schema_name: str) -> list[dict[str, object]]:
    schema = load(f"schemas/{schema_name}")
    if schema_name == "provider-manifest.schema.json":
        return schema["$defs"]["canonicalProviderIdentity"]["oneOf"]
    return schema["oneOf"]


def provider_identity_mapping(schema_name: str) -> set[tuple[str, str]]:
    return {
        (
            branch["properties"]["languageId"]["const"],
            branch["properties"]["providerId"]["const"],
        )
        for branch in provider_identity_branches(schema_name)
    }


def test_provider_register_conforms_to_asp_schema() -> None:
    schema = load("schemas/provider-register.schema.json")
    Draft202012Validator.check_schema(schema)
    schema_validator_for(ROOT / "schemas/provider-register.schema.json").validate(
        load("schemas/provider-register.json")
    )


def test_external_provider_is_discovered_only_from_provider_register(tmp_path: Path) -> None:
    register_path = tmp_path / "provider-register.json"
    identity = {
        "languageId": "external-test",
        "providerId": "asp-external-test",
    }
    register_path.write_text(
        json.dumps(
            {
                "schemaId": "agent.semantic-protocols.provider-register",
                "schemaVersion": "1",
                "providers": [identity],
            }
        )
    )

    register = json.loads(register_path.read_text())
    assert register["providers"] == [identity]
    assert "descriptor" not in register["providers"][0]


def test_provider_register_uses_canonical_language_identity() -> None:
    manifest_validator = provider_id_validator("provider-manifest.schema.json")
    identity_validator = provider_identity_validator()

    for identity in load("schemas/provider-register.json")["providers"]:
        manifest_validator.validate(identity["providerId"])
        identity_validator.validate(identity)
        assert "descriptor" not in identity


def test_live_corpus_lock_and_plan_derive_provider_id_from_language() -> None:
    lock = load("benchmarks/large-library-runtime-corpora.json")
    plan = load("benchmarks/live-corpus-search-query-qualification.json")

    locked = {
        entry["resourceId"]: entry["providerId"] for entry in lock["corpora"]
    }
    planned = {
        case["resourceId"]: case["providerId"]
        for case in plan["cases"]
    }

    assert planned == locked
    assert locked["org.worg"] == "orgize"
    assert locked["md.mdn-content"] == "orgize"


def test_canonical_provider_ids_are_used_by_the_public_corpus() -> None:
    canonical = {
        "asp-rust",
        "asp-python",
        "asp-typescript",
        "asp-julia",
        "asp-gerbil-scheme",
        "orgize",
    }
    lock = load("benchmarks/large-library-runtime-corpora.json")
    assert all(entry["providerId"] in canonical for entry in lock["corpora"])


def test_asp_client_server_bootstrap_uses_canonical_provider_identity() -> None:
    schema = load("schemas/asp-client-server-bootstrap.schema.json")
    fixture = load("schemas/fixtures/asp-client-server-bootstrap.ready.v1.json")
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(fixture)
    assert fixture["providerId"] == "asp-julia"


def test_provider_manifest_rejects_cross_language_identity() -> None:
    with pytest.raises(ValidationError):
        provider_identity_validator().validate(
            {"languageId": "rust", "providerId": "asp-python"}
        )


def test_every_public_contract_uses_the_canonical_provider_mapping() -> None:
    canonical = provider_identity_mapping("canonical-provider-identity.v1.schema.json")
    for schema_name in (
        "provider-manifest.schema.json",
        "asp-client-server-request.v1.schema.json",
        "asp-client-server-response.v1.schema.json",
        "asp-client-server-lifecycle-receipt.schema.json",
    ):
        assert provider_identity_mapping(schema_name) == canonical


def test_canonical_provider_identity_schema_accepts_every_registered_language() -> None:
    schema = load("schemas/canonical-provider-identity.v1.schema.json")
    identity_validator = Draft202012Validator(schema)
    for language_id, provider_id in provider_identity_mapping(
        "canonical-provider-identity.v1.schema.json"
    ):
        identity_validator.validate(
            {
                "schemaId": "agent.semantic-protocols.canonical-provider-identity",
                "schemaVersion": "1",
                "languageId": language_id,
                "providerId": provider_id,
            }
        )


def test_gerbil_structural_index_is_not_a_python_graphs_authority() -> None:
    structural_index = (
        ROOT
        / "languages/asp-gerbil-scheme/src/protocol/structural-index.ss"
    ).read_text(encoding="utf-8")

    assert "asp-python-graphs" not in structural_index
    assert "graphTurboOwner" not in structural_index
    assert "asp-rust" not in structural_index
    assert "heavyIndexOwner" not in structural_index
