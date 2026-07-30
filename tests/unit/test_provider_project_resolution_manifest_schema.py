from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
RUST_PROVIDER_MANIFEST = (
    ROOT
    / "languages"
    / "rust-lang-project-harness"
    / "provider"
    / "asp-provider-manifest.json"
)


def load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_shared_provider_manifest_schemas_reference_project_resolution() -> None:
    expected_ref = "provider-project-resolution-descriptor.v1.schema.json"
    for schema_name in (
        "provider-manifest.v1.schema.json",
        "semantic-agent-hook-provider-manifest.v1.schema.json",
    ):
        schema = load_json(SCHEMAS / schema_name)
        properties = schema["properties"]
        assert isinstance(properties, dict)
        assert properties["projectResolution"] == {"$ref": expected_ref}


def test_rust_provider_manifest_declares_typed_project_resolution() -> None:
    project_resolution_schema = load_json(
        SCHEMAS / "provider-project-resolution-descriptor.v1.schema.json"
    )
    manifest = load_json(RUST_PROVIDER_MANIFEST)

    descriptor = manifest["projectResolution"]
    assert isinstance(descriptor, dict)
    Draft202012Validator(project_resolution_schema).validate(descriptor)
    assert descriptor["capabilityId"] == "project-resolution"
    assert descriptor["parserId"] == "rust.cargo-toml"
    assert descriptor["commandBinding"] == "project-resolution-stdin"
    assert descriptor["supportsGitCandidates"] is True
    assert descriptor["supportsProviderOnly"] is False
