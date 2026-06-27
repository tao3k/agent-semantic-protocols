from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


REPO_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = REPO_ROOT / "schemas"


def test_sandtable_scenario_accepts_route_quality_contract() -> None:
    _validator("semantic-sandtable-scenario.v1.schema.json").validate(
        {
            "id": "route.known-owner-skips-prime",
            "language": "rust",
            "workdir": {"relative": "."},
            "evidence": {
                "source": "handwritten",
                "intent": "Known owner and symbol should not start at search prime",
                "scenarioQuality": _scenario_quality(),
                "routeVerification": _route_expectation(),
                "deepQuestionCases": [_deep_question_case()],
            },
            "steps": [
                {
                    "id": "owneritems",
                    "kind": "command",
                    "command": [
                        "asp",
                        "rust",
                        "search",
                        "owner",
                        "src/lib.rs",
                        "items",
                    ],
                }
            ],
        }
    )


def _deep_question_case() -> dict[str, Any]:
    return {
        "id": "knownowner",
        "question": "Inspect native prime owner-only rendering without re-mapping the workspace.",
        "stepIds": ["owneritems"],
        "queryTerms": ["native", "prime", "owner"],
        "audit": {
            "maxAspCommands": 3,
            "maxSearchCommands": 2,
            "maxQueryCommands": 1,
            "maxRepeatedCommands": 0,
            "requiresGraphSignals": True,
            "requiresHookEvents": True,
        },
        "expectedAspFlow": {
            "requiredStages": ["query-selector"],
            "forbiddenStages": ["search-prime"],
        },
        "scenarioQuality": _scenario_quality(),
        "routeVerification": _route_expectation(),
    }


def _scenario_quality() -> dict[str, Any]:
    return {
        "intentClear": True,
        "intentRouteAligned": True,
        "evidenceStateComplete": True,
        "forbiddenRoutesJustified": True,
        "oracleRoute": ["owner-items", "query-code"],
        "ambiguousIntent": False,
        "reviewer": "static-scenario-contract",
        "notes": ["known owner and symbol make prime broader than necessary"],
    }


def _route_expectation() -> dict[str, Any]:
    return {
        "expectedEvidenceAnchors": ["owner-path", "symbol"],
        "allowedFirstRoutes": ["owner-items", "owner-skeleton"],
        "forbiddenRoutes": ["prime", "direct-read"],
        "requiredRejectedRoutes": ["prime"],
        "forbiddenRiskFlags": ["unnecessary-prime", "executable-line-range"],
        "minScores": {
            "routeFaithfulness": 4,
            "routeEfficiency": 3,
            "semanticPrecision": 4,
            "fallbackDiscipline": 3,
            "verificationHonesty": 4,
            "finalAnswerGrounding": 3,
        },
        "requireRouteJustification": True,
        "requireExactCodeIdentity": True,
        "requireNoExecutableLineRange": True,
        "requireVerificationEvidence": True,
    }


def _validator(name: str) -> Draft202012Validator:
    return Draft202012Validator(_load_schema(name), registry=_schema_registry())


def _load_schema(name: str) -> dict[str, Any]:
    return json.loads((SCHEMA_DIR / name).read_text(encoding="utf-8"))


def _schema_registry() -> Registry:
    resources = []
    for schema_path in SCHEMA_DIR.glob("*.schema.json"):
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        schema_id = schema.get("$id")
        if schema_id:
            resources.append((schema_id, Resource.from_contents(schema)))
    return Registry().with_resources(resources)
