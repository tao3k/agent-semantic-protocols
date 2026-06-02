from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def load_schema(name: str) -> dict:
    return json.loads((ROOT / "schemas" / name).read_text(encoding="utf-8"))


def test_verification_receipt_schema_accepts_cargo_check_adapter_receipt() -> None:
    schema = load_schema("semantic-verification-receipt.v1.schema.json")
    validator = Draft202012Validator(schema)

    validator.validate(
        {
            "schemaId": "agent.semantic-protocols.semantic-verification-receipt",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.verification-receipt",
            "protocolVersion": "1",
            "receiptId": "rust.cargo-check.src-model",
            "producer": {
                "languageId": "rust",
                "providerId": "rs-harness",
                "adapterId": "rust.cargo-check",
                "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
            },
            "project": {
                "name": "rust-lang-project-harness",
                "workdir": ".",
                "package": "rust-lang-project-harness",
            },
            "tool": "cargo-check",
            "status": "passed",
            "command": {
                "argv": ["cargo", "check", "--message-format=json"],
                "workdir": ".",
                "outputFormat": "cargo-json",
            },
            "exitCode": 0,
            "durationMs": 423,
            "summary": "cargo check completed without compiler errors",
            "observations": [
                {
                    "kind": "exit-status",
                    "message": "exit code 0",
                    "fields": {"stdoutBytes": 1024, "stderrBytes": 0},
                }
            ],
            "candidateIds": ["agent-r027:src.model.rs:42"],
            "taskFingerprints": ["regression:src/model.rs"],
        }
    )


def test_verification_receipt_schema_lists_p1_and_future_adapters() -> None:
    schema = load_schema("semantic-verification-receipt.v1.schema.json")
    tool_values = schema["$defs"]["tool"]["enum"]

    assert tool_values[:4] == ["cargo-check", "cargo-test", "clippy", "expect-test"]
    assert {"proptest", "cargo-fuzz", "kani", "creusot", "verus"}.issubset(
        tool_values
    )
