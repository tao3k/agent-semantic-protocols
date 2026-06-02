from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def load_schema() -> dict:
    return json.loads(
        (ROOT / "schemas" / "semantic-determinism-readiness.v1.schema.json").read_text(
            encoding="utf-8"
        )
    )


def test_determinism_readiness_schema_accepts_direct_clock_observation() -> None:
    schema = load_schema()
    validator = Draft202012Validator(schema)

    validator.validate(
        {
            "schemaId": "agent.semantic-protocols.semantic-determinism-readiness",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.determinism-readiness",
            "protocolVersion": "1",
            "readinessId": "rust.determinism-readiness.project",
            "producer": {
                "languageId": "rust",
                "providerId": "rs-harness",
                "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
            },
            "project": {"root": "."},
            "status": "needs-injection",
            "observations": [
                {
                    "observationId": "clock:src-lib.rs:10",
                    "category": "clock",
                    "evidenceKind": "function-call",
                    "severity": "warning",
                    "summary": "Direct clock read uses SystemTime::now.",
                    "path": "src/lib.rs",
                    "line": 10,
                    "symbol": "now",
                    "expression": "std::time::SystemTime::now",
                    "sourceLine": "let now = std::time::SystemTime::now();",
                    "direct": True,
                }
            ],
            "suggestions": [
                {
                    "kind": "trait-injection",
                    "category": "clock",
                    "message": "Inject a clock provider trait instead of reading time directly.",
                    "path": "src/lib.rs",
                    "line": 10,
                    "traitName": "Clock",
                }
            ],
        }
    )


def test_determinism_readiness_rejects_absolute_observation_paths() -> None:
    schema = load_schema()
    validator = Draft202012Validator(schema)

    errors = list(
        validator.iter_errors(
            {
                "schemaId": "agent.semantic-protocols.semantic-determinism-readiness",
                "schemaVersion": "1",
                "protocolId": "agent.semantic-protocols.determinism-readiness",
                "protocolVersion": "1",
                "readinessId": "rust.determinism-readiness.project",
                "producer": {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
                },
                "project": {"root": "."},
                "status": "needs-injection",
                "observations": [
                    {
                        "observationId": "env:/tmp/lib.rs:1",
                        "category": "environment",
                        "evidenceKind": "function-call",
                        "severity": "warning",
                        "summary": "Direct environment read.",
                        "path": "/tmp/lib.rs",
                        "line": 1,
                        "direct": True,
                    }
                ],
                "suggestions": [],
            }
        )
    )

    assert errors
