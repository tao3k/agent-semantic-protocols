import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[2]
FORBIDDEN_STATIC_SCOPE_KEYS = {
    "defaultSourceRoots",
    "defaultIgnoredPathPrefixes",
}
PROVIDER_MANIFESTS = (
    ROOT
    / "languages"
    / "rust-lang-project-harness"
    / "provider"
    / "asp-provider-manifest.json",
    ROOT
    / "languages"
    / "python-lang-project-harness"
    / "provider"
    / "asp-provider-manifest.json",
    ROOT
    / "languages"
    / "typescript-lang-project-harness"
    / "provider"
    / "asp-provider-manifest.json",
    ROOT
    / "languages"
    / "gerbil-scheme-language-project-harness"
    / "provider"
    / "asp-provider-manifest.json",
    ROOT
    / "languages"
    / "JuliaLangProjectHarness.jl"
    / "juliac"
    / "asp-provider-manifest.json",
)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def test_shared_provider_schemas_forbid_static_path_scope() -> None:
    schema_owners = (
        (
            ROOT / "schemas" / "provider-manifest.v1.schema.json",
            "manifestSourceDefaults",
        ),
        (
            ROOT
            / "schemas"
            / "semantic-agent-hook-provider-manifest.v1.schema.json",
            "sourceDefaults",
        ),
    )

    for schema_path, definition_name in schema_owners:
        schema = load_json(schema_path)
        source_properties = schema["$defs"][definition_name]["properties"]
        assert FORBIDDEN_STATIC_SCOPE_KEYS.isdisjoint(source_properties)


def test_registered_provider_manifests_do_not_publish_static_path_scope() -> None:
    for manifest_path in PROVIDER_MANIFESTS:
        manifest = load_json(manifest_path)
        assert FORBIDDEN_STATIC_SCOPE_KEYS.isdisjoint(manifest["source"]), manifest_path


def test_static_path_scope_is_rejected_instead_of_ignored() -> None:
    schema = load_json(ROOT / "schemas" / "provider-manifest.v1.schema.json")
    source_schema = {
        "$schema": schema["$schema"],
        "$defs": schema["$defs"],
        **schema["$defs"]["manifestSourceDefaults"],
    }
    source = load_json(PROVIDER_MANIFESTS[0])["source"]
    source["defaultSourceRoots"] = ["src"]

    errors = list(jsonschema.Draft202012Validator(source_schema).iter_errors(source))
    assert any(
        error.validator == "additionalProperties"
        and "defaultSourceRoots" in error.message
        for error in errors
    )
