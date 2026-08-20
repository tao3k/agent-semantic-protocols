from pathlib import Path

import json
import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]

EXTERNAL_PROVIDERS = (
    ("languages/rust-lang-project-harness/provider", "rust"),
    ("languages/python-lang-project-harness/provider", "python"),
    ("languages/typescript-lang-project-harness/provider", "typescript"),
    ("languages/gerbil-scheme-language-project-harness/provider", "gerbil-scheme"),
    ("languages/JuliaLangProjectHarness.jl/juliac", "julia"),
)


def load(relative: str) -> dict[str, object]:
    return json.loads((ROOT / relative).read_text())


def provider_id_validator(schema_name: str) -> Draft202012Validator:
    schema = load(f"schemas/{schema_name}")
    provider_id = schema["properties"]["providerId"]
    return Draft202012Validator(provider_id)


def provider_identity_validator() -> Draft202012Validator:
    schema = load("schemas/provider-manifest.v1.schema.json")
    identity = schema["$defs"]["canonicalProviderIdentity"]
    return Draft202012Validator(identity)


def install_identity_validator() -> Draft202012Validator:
    schema = load("schemas/provider-workspace-install.v1.schema.json")
    return Draft202012Validator({"oneOf": schema["oneOf"]})


def provider_identity_branches(schema_name: str) -> list[dict[str, object]]:
    schema = load(f"schemas/{schema_name}")
    if schema_name == "provider-manifest.v1.schema.json":
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


def test_external_provider_manifests_and_install_receipts_use_language_identity() -> None:
    manifest_validator = provider_id_validator("provider-manifest.v1.schema.json")
    install_validator = provider_id_validator("provider-workspace-install.v1.schema.json")
    identity_validator = provider_identity_validator()
    workspace_install_identity = install_identity_validator()

    for provider_root, language_id in EXTERNAL_PROVIDERS:
        expected = f"asp-{language_id}"
        manifest = load(f"{provider_root}/asp-provider-manifest.json")
        install = load(f"{provider_root}/asp-provider-workspace-install.json")
        assert manifest["providerId"] == expected
        assert install["languageId"] == language_id
        assert install["providerId"] == expected
        manifest_validator.validate(manifest["providerId"])
        install_validator.validate(install["providerId"])
        identity_validator.validate(manifest)
        workspace_install_identity.validate(install)


def test_live_corpus_lock_and_plan_derive_provider_id_from_language() -> None:
    lock = load("benchmarks/large-library-runtime-corpora.v1.json")
    plan = load("benchmarks/live-corpus-search-query-qualification.v1.json")

    locked = {
        entry["resourceId"]: f"asp-{entry['language']}"
        for entry in lock["corpora"]
    }
    planned = {
        case["resourceId"]: case["providerId"]
        for case in plan["cases"]
    }

    assert planned == locked
    assert locked["org.worg"] == "asp-org"
    assert locked["md.mdn-content"] == "asp-md"


def test_implementation_names_are_not_public_provider_ids() -> None:
    legacy = {
        "rs-harness",
        "py-harness",
        "ts-harness",
        "gerbil-scheme-harness",
        "julia-lang-project-harness",
        "orgize",
        "asp+rust",
    }
    lock = load("benchmarks/large-library-runtime-corpora.v1.json")
    assert legacy.isdisjoint(entry["providerId"] for entry in lock["corpora"])


def test_asp_client_server_bootstrap_uses_canonical_provider_identity() -> None:
    schema = load("schemas/asp-client-server-bootstrap.v1.schema.json")
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
        "provider-manifest.v1.schema.json",
        "asp-client-server-request.v1.schema.json",
        "asp-client-server-response.v1.schema.json",
        "asp-client-server-lifecycle-receipt.v1.schema.json",
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
