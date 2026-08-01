import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "provider-manifest.v1.schema.json"
MANIFESTS = (
    ROOT / "languages/rust-lang-project-harness/provider/asp-provider-manifest.json",
    ROOT / "languages/typescript-lang-project-harness/provider/asp-provider-manifest.json",
    ROOT / "languages/python-lang-project-harness/provider/asp-provider-manifest.json",
    ROOT
    / "languages/gerbil-scheme-language-project-harness/provider/asp-provider-manifest.json",
    ROOT / "languages/JuliaLangProjectHarness.jl/juliac/asp-provider-manifest.json",
    ROOT / "languages/orgize/provider/asp-org-provider-manifest.json",
    ROOT / "languages/orgize/provider/asp-md-provider-manifest.json",
)


def load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def development_validator() -> Draft202012Validator:
    schema = load_json(SCHEMA_PATH)
    return Draft202012Validator(schema["$defs"]["providerDevelopmentDescriptor"])


def test_all_registered_manifests_have_valid_v1_development_authority() -> None:
    validator = development_validator()
    for manifest_path in MANIFESTS:
        manifest = load_json(manifest_path)
        validator.validate(manifest["development"])


@pytest.mark.parametrize(
    "source_root",
    (
        "/absolute/provider",
        "../provider",
        "languages/../provider",
        "languages/provider/..",
    ),
)
def test_development_source_root_rejects_absolute_and_parent_escape(
    source_root: str,
) -> None:
    descriptor = {
        "schemaId": "agent.semantic-protocols.provider-development-descriptor",
        "schemaVersion": "1",
        "sourceRoot": source_root,
        "buildBinding": "root-development-installer-v1",
        "artifactDomain": "checkout",
    }
    with pytest.raises(ValidationError):
        development_validator().validate(descriptor)


def test_development_descriptor_rejects_unknown_binding_and_domain() -> None:
    descriptor = {
        "schemaId": "agent.semantic-protocols.provider-development-descriptor",
        "schemaVersion": "1",
        "sourceRoot": "languages/provider",
        "buildBinding": "legacy-installer",
        "artifactDomain": "path-fallback",
    }
    with pytest.raises(ValidationError):
        development_validator().validate(descriptor)
