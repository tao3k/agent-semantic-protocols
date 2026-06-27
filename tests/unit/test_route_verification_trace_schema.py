from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


REPO_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = REPO_ROOT / "schemas"


def _load_schema(name: str) -> dict:
    return json.loads((SCHEMA_DIR / name).read_text())


def _schema_registry() -> Registry:
    resources = []
    for schema_path in SCHEMA_DIR.glob("*.schema.json"):
        schema = json.loads(schema_path.read_text())
        schema_id = schema.get("$id")
        if schema_id:
            resources.append((schema_id, Resource.from_contents(schema)))
    return Registry().with_resources(resources)


def test_route_verification_trace_schema_accepts_evidence_state_routing() -> None:
    schema = _load_schema("semantic-route-verification-trace.v1.schema.json")
    validator = Draft202012Validator(schema, registry=_schema_registry())

    validator.validate(
        {
            "schemaId": "agent.semantic-protocols.semantic-route-verification-trace",
            "schemaVersion": "1",
            "verifierVersion": "asp-route-verifier.v1",
            "monitorPatternSetVersion": "2026-06-27",
            "evidenceState": {
                "anchors": ["owner-path", "symbol"],
                "ownerPath": "crates/agent-semantic-client/src/native_prime.rs",
                "query": "native prime owner-only frontier",
                "evidenceRefs": ["user-anchor:native_prime.rs"],
            },
            "chosenRoute": {
                "route": "owner-skeleton",
                "reason": "owner and symbol anchors exist, so workspace prime is broader than necessary",
                "evidenceRefs": ["evidenceState.ownerPath"],
            },
            "rejectedRoutes": [
                {
                    "route": "prime",
                    "reason": "owner evidence exists",
                    "risk": "unnecessary-prime",
                }
            ],
            "routePlan": [
                {
                    "route": "owner-skeleton",
                    "preconditions": ["owner-path"],
                    "expectedProjection": "skeleton",
                    "codePolicy": "disabled",
                    "reason": "collect structure before exact code",
                },
                {
                    "route": "query-code",
                    "preconditions": ["exact-selector"],
                    "expectedProjection": "code",
                    "codePolicy": "exact-only",
                    "reason": "read code only after exact parser identity",
                },
            ],
            "executedTrace": [
                {
                    "commandId": "c1",
                    "route": "owner-skeleton",
                    "projection": "skeleton",
                    "codePolicy": "disabled",
                    "evidenceRefs": ["command:c1"],
                }
            ],
            "behaviorScores": {
                "routeFaithfulness": 4,
                "routeEfficiency": 4,
                "semanticPrecision": 4,
                "fallbackDiscipline": 4,
                "verificationHonesty": 4,
                "finalAnswerGrounding": 3,
            },
            "riskFlags": [],
            "routeRegret": 0,
            "feedbackSignals": [
                {
                    "reason": "inefficiency",
                    "polarity": "negative",
                    "confidence": 0.9,
                    "evidenceRefs": ["user-feedback:avoid-prime-when-owner-known"],
                }
            ],
        }
    )


def test_sandtable_receipt_accepts_route_verification_trace() -> None:
    schema = _load_schema("semantic-sandtable-receipt.v1.schema.json")
    validator = Draft202012Validator(schema, registry=_schema_registry())

    validator.validate(
        {
            "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
            "schemaVersion": "1",
            "scenarioId": "route.known-owner-skips-prime",
            "language": "rust",
            "project": {
                "name": "fixture",
                "workdir": ".",
            },
            "intent": "Route known owner evidence without prime",
            "editBoundary": "after-search",
            "commands": [
                {
                    "id": "c1",
                    "kind": "search",
                    "argv": ["asp", "rust", "search", "owner", "src/lib.rs", "items"],
                    "metrics": {
                        "elapsedMs": 1,
                        "stdoutBytes": 10,
                        "stderrBytes": 0,
                    },
                }
            ],
            "summary": {
                "commandCount": 1,
                "stdoutBytes": 10,
                "stderrBytes": 0,
                "elapsedMs": 1,
                "aspCommands": 1,
                "searchCommands": 1,
            },
            "routeVerificationTrace": {
                "schemaId": "agent.semantic-protocols.semantic-route-verification-trace",
                "schemaVersion": "1",
                "verifierVersion": "asp-route-verifier.v1",
                "monitorPatternSetVersion": "2026-06-27",
                "evidenceState": {
                    "anchors": ["owner-path"],
                    "ownerPath": "src/lib.rs",
                },
                "chosenRoute": {
                    "route": "owner-items",
                    "reason": "owner path was already known",
                },
                "routePlan": [
                    {
                        "route": "owner-items",
                        "preconditions": ["owner-path"],
                        "expectedProjection": "names",
                        "codePolicy": "disabled",
                        "reason": "inspect item frontier before code",
                    }
                ],
                "executedTrace": [
                    {
                        "commandId": "c1",
                        "route": "owner-items",
                        "projection": "names",
                        "codePolicy": "disabled",
                    }
                ],
                "behaviorScores": {
                    "routeFaithfulness": 4,
                    "routeEfficiency": 4,
                    "semanticPrecision": 4,
                    "fallbackDiscipline": 4,
                    "verificationHonesty": 4,
                    "finalAnswerGrounding": 4,
                },
                "riskFlags": [],
            },
        }
    )


def test_sandtable_scenario_accepts_route_verification_expectations() -> None:
    schema = _load_schema("semantic-sandtable-scenario.v1.schema.json")
    validator = Draft202012Validator(schema, registry=_schema_registry())

    route_expectation = {
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

    validator.validate(
        {
            "id": "route.known-owner-skips-prime",
            "language": "rust",
            "workdir": {
                "relative": ".",
            },
            "evidence": {
                "source": "handwritten",
                "intent": "Known owner and symbol should not start at search prime",
                "routeVerification": route_expectation,
                "deepQuestionCases": [
                    {
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
                        "routeVerification": route_expectation,
                    }
                ],
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
