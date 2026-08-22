from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


REPO_ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATHS = (
    "languages/rust-lang-project-harness/schemas/asp-provider.json",
    "languages/typescript-lang-project-harness/schemas/asp-provider.json",
    "languages/python-lang-project-harness/schemas/asp-provider.json",
    "languages/JuliaLangProjectHarness.jl/schemas/asp-provider.json",
    "languages/gerbil-scheme-language-project-harness/schemas/asp-provider.json",
    "languages/orgize/schemas/asp-org-provider.json",
    "languages/orgize/schemas/asp-md-provider.json",
)


def _document(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _schema_registry() -> Registry:
    resources = []
    for path in (REPO_ROOT / "schemas").glob("*.schema.json"):
        document = _document(path)
        schema_id = document.get("$id")
        if isinstance(schema_id, str):
            resources.append((schema_id, Resource.from_contents(document)))
        resources.append((path.name, Resource.from_contents(document)))
    return Registry().with_resources(resources)


def test_provider_manifests_use_one_typed_resolution_owner() -> None:
    schema = _document(REPO_ROOT / "schemas/provider-manifest.schema.json")
    validator = Draft202012Validator(schema, registry=_schema_registry())

    for relative_path in MANIFEST_PATHS:
        manifest = _document(REPO_ROOT / relative_path)
        errors = sorted(validator.iter_errors(manifest), key=lambda error: error.json_path)
        assert not errors, (
            f"{relative_path}: "
            + "; ".join(f"{error.json_path}: {error.message}" for error in errors)
        )
        assert "source" not in manifest, relative_path
        search_capabilities = manifest.get("searchCapabilities", {})
        assert "workspaceScope" not in search_capabilities, relative_path
        route_bindings = manifest.get("routeBindings", {})
        assert "workspaceScope" not in route_bindings, relative_path
        assert ("projectResolution" in manifest) ^ (
            "documentResolution" in manifest
        ), relative_path
