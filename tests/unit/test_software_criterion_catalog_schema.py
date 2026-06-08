"""Validate the software criterion catalog schema."""

from __future__ import annotations

import json
import sys
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "tests" / "unit"))

from schema_validation import schema_validator_for  # noqa: E402

_SCHEMA_PATH = _ROOT / "schemas" / "software-criterion-catalog.v1.schema.json"
_CATALOG_PATH = _ROOT / "schemas" / "software-criteria.v1.json"
_REGISTRY_PATH = _ROOT / "schemas" / "semantic-language-registry.providers.v1.json"


def _load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_software_criterion_catalog_schema_is_valid() -> None:
    Draft202012Validator.check_schema(_load_json(_SCHEMA_PATH))


def test_software_criterion_catalog_records_provider_readiness() -> None:
    catalog = _load_json(_CATALOG_PATH)

    schema_validator_for(_SCHEMA_PATH).validate(catalog)

    provider_status = {
        mapping["languageId"]: mapping["status"]
        for mapping in catalog["providerMappings"]
    }
    assert provider_status == {
        "python": "ready",
        "rust": "ready",
        "typescript": "ready",
        "julia": "planned",
    }

    criterion_ids = {criterion["criterionId"] for criterion in catalog["criteria"]}
    assert {
        "control-flow.decision-stack",
        "control-flow.traversal-knot",
        "control-flow.literal-dispatch-chain",
        "control-flow.broad-linear-phase",
        "native-idiom.manual-transform-loop",
        "error-resource.hidden-boundary",
    } <= criterion_ids

    provider_criterion_ids = {
        mapping["languageId"]: set(mapping["criterionIds"])
        for mapping in catalog["providerMappings"]
    }
    assert provider_criterion_ids["typescript"] == {
        "control-flow.decision-stack",
        "control-flow.traversal-knot",
        "control-flow.literal-dispatch-chain",
        "control-flow.broad-linear-phase",
        "native-idiom.manual-transform-loop",
    }


def test_source_language_registry_advertises_software_criterion_catalog_schema() -> None:
    registry = _load_json(_REGISTRY_PATH)
    source_languages = {"rust", "typescript", "python", "julia"}
    registrations = {
        language["languageId"]: {schema["schemaId"] for schema in language["schemas"]}
        for language in registry["languages"]
        if language["languageId"] in source_languages
    }

    assert set(registrations) == source_languages
    for schema_ids in registrations.values():
        assert "agent.semantic-protocols.software-criterion-catalog" in schema_ids


def test_software_criterion_catalog_rejects_legacy_naming_lane() -> None:
    combined_contract = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (_SCHEMA_PATH, _CATALOG_PATH, _REGISTRY_PATH)
    )

    forbidden = (
        "agent-" + "quality-" + "signal",
        "agent-" + "coding-" + "quality",
        "agent" + "Quality" + "Signals",
        "signal" + "SetId",
        "signal" + "Id",
        "signal" + "Ids",
    )
    for token in forbidden:
        assert token not in combined_contract
