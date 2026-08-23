import json
from dataclasses import dataclass
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
REQUIRED_CLIENT_SCHEMAS = {
    "asp-client-protocol-catalog.schema.json",
    "asp-client-frame.schema.json",
    "asp-client-conformance.schema.json",
}
REQUIRED_PROVIDER_SCHEMAS = {
    "provider-registration.schema.json",
    "provider-route.schema.json",
    "provider-query-pack-descriptor.schema.json",
    "provider-runtime-contract-descriptor.schema.json",
    "asp-client-server-descriptor.schema.json",
    "provider-workspace-install.schema.json",
    "language-schema-bundle-receipt.schema.json",
}


@dataclass(frozen=True)
class ProviderPackage:
    language_id: str
    package_root: Path
    descriptor_root: Path


PROVIDERS = (
    ProviderPackage("rust", REPOSITORY_ROOT / "languages/rust-lang-project-harness", Path("provider")),
    ProviderPackage("typescript", REPOSITORY_ROOT / "languages/typescript-lang-project-harness", Path("provider")),
    ProviderPackage("python", REPOSITORY_ROOT / "languages/python-lang-project-harness", Path("provider")),
    ProviderPackage("julia", REPOSITORY_ROOT / "languages/JuliaLangProjectHarness.jl", Path("juliac")),
    ProviderPackage("gerbil-scheme", REPOSITORY_ROOT / "languages/gerbil-scheme-language-project-harness", Path("provider")),
)


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def test_language_schema_bundles_publish_client_and_provider_protocols() -> None:
    for provider in PROVIDERS:
        receipt_path = provider.package_root / "schemas/.asp-schema-manager-receipt.json"
        receipt = load_json(receipt_path)
        assert isinstance(receipt, dict)
        assert receipt["schemaId"] == "agent.semantic-protocols.language-schema-bundle-receipt"
        assert receipt["schemaVersion"] == "1"
        assert receipt["languageId"] == provider.language_id
        assert receipt["bundleDigest"].startswith("blake3-256:")
        names = {entry["name"] for entry in receipt["schemas"]}
        assert REQUIRED_CLIENT_SCHEMAS <= names
        assert REQUIRED_PROVIDER_SCHEMAS <= names


def test_provider_registration_is_the_only_package_local_wire_authority() -> None:
    for provider in PROVIDERS:
        descriptor_root = provider.package_root / provider.descriptor_root
        workspace_path = descriptor_root / "asp-provider-workspace-install.json"
        workspace = load_json(workspace_path)
        assert isinstance(workspace, dict)
        registration_path = workspace_path.parent / workspace["providerRegistration"]
        registration = load_json(registration_path)
        assert isinstance(registration, dict)
        assert registration["languageId"] == provider.language_id
        assert registration["runtimeContract"]["transport"] == "http-json"
        assert registration["runtimeContract"]["clientBinding"] == "schema-driven"
        operations = registration["runtimeContract"]["operations"]
        assert {item["operation"] for item in operations} >= {
            "projection-batch",
            "project-resolution",
        }
        for operation in operations:
            assert operation["requestSchemaId"].startswith(
                "https://schemas.agent-semantic-protocols.dev/"
            )
            assert operation["responseSchemaId"].startswith(
                "https://schemas.agent-semantic-protocols.dev/"
            )
