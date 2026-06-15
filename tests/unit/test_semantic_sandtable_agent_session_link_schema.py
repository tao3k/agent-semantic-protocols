"""Validate sandtable scenario evidence links to agent-session receipts."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA = json.loads(
    (_REPO_ROOT / "schemas/semantic-sandtable-scenario.v1.schema.json").read_text()
)


def test_live_agent_session_receipt_link_evidence_is_valid() -> None:
    scenario: dict[str, object] = {
        "id": "rust.tokio-agent-observability",
        "language": "rust",
        "workdir": ".",
        "evidence": {
            "source": "live-agent",
            "intent": "Explain Tokio IO readiness.",
            "receiptPath": "receipts/sandtable-receipt.json",
            "agentSessionId": "session-1",
            "agentSessionReceiptPath": "receipts/agent-session-receipt.json",
            "answer": {
                "present": True,
                "afterLastToolUse": True,
                "textBytes": 80,
                "textLineCount": 1,
                "groundingStatus": "grounded",
                "evidenceRefs": ["command-call_1"],
            },
            "qualityFindings": [
                {
                    "id": "command.repeated",
                    "kind": "command-efficiency",
                    "severity": "warning",
                    "message": "Repeated command argv were recorded.",
                }
            ],
        },
        "steps": [{"id": "noop", "command": ["true"]}],
    }

    Draft202012Validator(_SCHEMA).validate(scenario)
