from __future__ import annotations

import json
from pathlib import Path



ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"


def load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_shared_provider_manifest_schemas_reference_project_resolution() -> None:
    expected_ref = (
        "https://schemas.agent-semantic-protocols.dev/"
        "provider-project-resolution-descriptor.schema.json"
    )
    for schema_name in (
        "provider-manifest.schema.json",
    ):
        schema = load_json(SCHEMAS / schema_name)
        properties = schema["properties"]
        assert isinstance(properties, dict)
        assert properties["projectResolution"] == {"$ref": expected_ref}
