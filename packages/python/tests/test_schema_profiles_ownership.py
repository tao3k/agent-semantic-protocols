from __future__ import annotations

import json

from tools.schema_profile_catalog import LanguageSchemaProfile
from tools.schema_profiles import schema_profile_changes


def test_sync_removes_only_root_managed_schema_files(tmp_path) -> None:
    root_schemas = tmp_path / "schemas"
    provider_schemas = tmp_path / "languages" / "rust" / "schemas"
    root_schemas.mkdir()
    provider_schemas.mkdir(parents=True)

    schema_name = "provider-manifest.schema.json"
    schema = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": f"https://schemas.agent-semantic-protocols.dev/{schema_name}",
        "type": "object",
    }
    encoded_schema = json.dumps(schema)
    (root_schemas / schema_name).write_text(encoded_schema, encoding="utf-8")
    (provider_schemas / schema_name).write_text(encoded_schema, encoding="utf-8")
    (provider_schemas / "legacy.v1.schema.json").write_text("{}", encoding="utf-8")
    (provider_schemas / "asp-provider.json").write_text("{}", encoding="utf-8")
    (provider_schemas / "asp-registration.json").write_text("{}", encoding="utf-8")

    profile = LanguageSchemaProfile(
        language_id="rust",
        package_root="languages/rust",
        shared_schema_files=(schema_name,),
        provider_schema_files=(),
    )
    changes = schema_profile_changes(tmp_path, profiles=(profile,))

    assert not any(
        change.schema_name in {"asp-provider.json", "asp-registration.json"}
        for change in changes
    )
    assert any(
        change.action == "remove" and change.schema_name == "legacy.v1.schema.json"
        for change in changes
    )
