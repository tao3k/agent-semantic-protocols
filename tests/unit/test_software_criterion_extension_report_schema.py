"""Validate the software criterion extension report envelope."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "tests" / "unit"))

from schema_validation import schema_validator_for  # noqa: E402

_SCHEMA_PATH = _ROOT / "schemas" / "software-criterion-extension-report.v1.schema.json"
_REGISTRY_PATH = _ROOT / "schemas" / "semantic-language-registry.providers.v1.json"
_RFC_PATH = (
    _ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.16-software-criterion-extension-policy.org"
)


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _effect_report() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.software-criterion-extension-report",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.software-criterion",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "projectRoot": ".",
        "extensionId": "typescript.extension.effect",
        "ecosystem": "effect",
        "mode": "strict",
        "profile": "production-service",
        "activationEvidence": [
            {
                "kind": "project-config",
                "summary": "typescript.extension.effect mode=strict",
                "locator": {"path": "package.json", "line": 12},
            },
            {
                "kind": "dependency",
                "summary": "package dependency effect",
                "fields": {"package": "effect"},
            },
        ],
        "findings": [
            {
                "ruleId": "typescript.extension.effect.resource.acquire-without-scope",
                "criterionDomain": "resource-lifetime",
                "severity": "warning",
                "summary": "source owner acquires an external client without Effect scope",
                "agentRisk": "agent cannot prove cleanup on failure or interruption",
                "ownerPath": "src/services/user.ts",
                "locator": {"path": "src/services/user.ts", "lineRange": "14:28"},
                "facts": {
                    "acquireCall": "createClient",
                    "scopeEvidence": False,
                    "layerEvidence": False,
                },
                "repair": [
                    {
                        "recipeId": "effect.resource.acquire-release",
                        "summary": "wrap acquisition in Effect.acquireRelease",
                        "target": "Effect.acquireRelease",
                    },
                    {
                        "recipeId": "effect.resource.layer",
                        "summary": "expose the client through a scoped Layer service",
                        "target": "Layer.scoped",
                    },
                ],
                "sourceDoctrine": [
                    {
                        "kind": "official-docs",
                        "title": "Effect resource management",
                        "url": "https://effect.website/docs/resource-management/introduction/",
                        "claim": "Effect resource constructs make acquire/release visible.",
                    }
                ],
            }
        ],
    }


def test_software_criterion_extension_report_schema_is_valid() -> None:
    Draft202012Validator.check_schema(_load_json(_SCHEMA_PATH))


def test_software_criterion_extension_report_accepts_effect_strict_packet() -> None:
    schema_validator_for(_SCHEMA_PATH).validate(_effect_report())


def test_software_criterion_extension_report_accepts_detect_candidate_without_findings() -> None:
    report = _effect_report()
    report["mode"] = "detect"
    report["findings"] = []

    schema_validator_for(_SCHEMA_PATH).validate(report)


def test_software_criterion_extension_report_rejects_off_mode_findings() -> None:
    report = _effect_report()
    report["mode"] = "off"

    errors = list(schema_validator_for(_SCHEMA_PATH).iter_errors(report))

    assert errors
    assert any(list(error.path) == ["findings"] for error in errors)


def test_software_criterion_extension_report_rejects_generic_mapping_rule_id() -> None:
    report = _effect_report()
    report["findings"][0]["ruleId"] = "concurrency.no-backpressure"

    errors = list(schema_validator_for(_SCHEMA_PATH).iter_errors(report))

    assert errors
    assert any(list(error.path) == ["findings", 0, "ruleId"] for error in errors)


def test_source_language_registry_advertises_software_criterion_extension_schema() -> None:
    registry = _load_json(_REGISTRY_PATH)
    source_languages = {"rust", "typescript", "python", "julia"}
    registrations = {
        language["languageId"]: {schema["schemaId"] for schema in language["schemas"]}
        for language in registry["languages"]
        if language["languageId"] in source_languages
    }

    assert set(registrations) == source_languages
    for schema_ids in registrations.values():
        assert (
            "agent.semantic-protocols.software-criterion-extension-report" in schema_ids
        )


def test_software_criterion_extension_policy_rfc_locks_provider_owned_boundary() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")
    normalized_text = re.sub(r"\s+", " ", text)

    assert "core software criteria canon -> optional ecosystem extension" in normalized_text
    assert (
        "optional provider-owned extension or profile criterion packs"
        in normalized_text
    )
    assert "The root protocol must not model large-library practices as a cross-language" in text
    assert "typescript.extension.effect" in text
    assert "rust.extension.tokio" in text
    assert "julia.extension.moshi" in text
    assert "julia.profile.sciml" in text
    assert "agent-" + "quality-" + "signal" not in text
    assert "agent-" + "coding-" + "quality" not in text
