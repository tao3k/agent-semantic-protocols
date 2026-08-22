import json
from pathlib import Path

from tests.unit.schema_validator_support import local_schema_validator


ROOT = Path(__file__).resolve().parents[2]
FORBIDDEN_STATIC_SCOPE_KEYS = {
    "defaultSourceRoots",
    "defaultIgnoredPathPrefixes",
}
PROVIDER_MANIFESTS = (
    ROOT
    / "languages"
    / "rust-lang-project-harness"
    / "schemas"
    / "asp-provider.json",
    ROOT
    / "languages"
    / "python-lang-project-harness"
    / "schemas"
    / "asp-provider.json",
    ROOT
    / "languages"
    / "typescript-lang-project-harness"
    / "schemas"
    / "asp-provider.json",
    ROOT
    / "languages"
    / "gerbil-scheme-language-project-harness"
    / "schemas"
    / "asp-provider.json",
    ROOT
    / "languages"
    / "JuliaLangProjectHarness.jl"
    / "schemas"
    / "asp-provider.json",
)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def test_shared_provider_schemas_forbid_static_path_scope() -> None:
    provider_schema = load_json(ROOT / "schemas" / "provider-manifest.schema.json")
    assert "source" not in provider_schema["properties"]

    hook_schema = load_json(
        ROOT / "schemas" / "semantic-agent-hook-provider-manifest.schema.json"
    )
    source_properties = hook_schema["$defs"]["sourceDefaults"]["properties"]
    assert FORBIDDEN_STATIC_SCOPE_KEYS.isdisjoint(source_properties)


def test_registered_provider_manifests_do_not_publish_static_path_scope() -> None:
    for manifest_path in PROVIDER_MANIFESTS:
        manifest = load_json(manifest_path)
        assert "source" not in manifest, manifest_path


def test_static_path_scope_is_rejected_instead_of_ignored() -> None:
    manifest = load_json(PROVIDER_MANIFESTS[0])
    manifest["source"] = {"defaultSourceRoots": ["src"]}
    validator = local_schema_validator(
        ROOT / "schemas" / "provider-manifest.schema.json",
        ROOT / "schemas" / "provider-project-resolution-descriptor.schema.json",
        ROOT / "schemas" / "provider-runtime-contract-descriptor.schema.json",
        ROOT / "schemas" / "asp-client-server-descriptor.schema.json",
        ROOT / "schemas" / "provider-query-pack-descriptor.schema.json",
    )

    errors = list(validator.iter_errors(manifest))
    assert any(
        error.validator == "additionalProperties"
        and "source" in error.message
        for error in errors
    )
