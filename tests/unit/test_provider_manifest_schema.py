from __future__ import annotations

import json
from pathlib import Path

from .schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "provider-manifest.schema.json"
PROVIDER_MANIFESTS = (
    ROOT / "languages" / "rust-lang-project-harness" / "schemas" / "asp-provider.json",
    ROOT / "languages" / "python-lang-project-harness" / "schemas" / "asp-provider.json",
    ROOT / "languages" / "JuliaLangProjectHarness.jl" / "schemas" / "asp-provider.json",
    ROOT
    / "languages"
    / "gerbil-scheme-language-project-harness"
    / "schemas"
    / "asp-provider.json",
    ROOT
    / "languages"
    / "typescript-lang-project-harness"
    / "schemas"
    / "asp-provider.json",
)


def test_all_provider_manifests_implement_the_shared_schema() -> None:
    validator = schema_validator_for(SCHEMA_PATH)

    for manifest_path in PROVIDER_MANIFESTS:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        validator.validate(manifest)


def test_runtime_contract_is_the_only_operation_transport_authority() -> None:
    for manifest_path in PROVIDER_MANIFESTS:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        assert "languageProjection" not in manifest

        project_resolution = manifest.get("projectResolution")
        if project_resolution is not None:
            assert "commandBinding" not in project_resolution
            assert "requestSchema" not in project_resolution
            assert "responseSchema" not in project_resolution

        runtime_contract = manifest["runtimeContract"]
        assert runtime_contract["transport"] == "http-json"
        operations = [operation["operation"] for operation in runtime_contract["operations"]]
        assert "projection-batch" in operations
        assert "project-resolution" in operations
        if manifest["languageId"] == "rust":
            assert "syntax-query" in operations
