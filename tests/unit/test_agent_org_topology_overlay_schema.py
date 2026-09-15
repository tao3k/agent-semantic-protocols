# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def test_agent_org_topology_overlay_v1_contract_accepts_separate_identities() -> None:
    schema = json.loads(
        (ROOT / "schemas/agent-org-topology-overlay.v1.schema.json").read_text()
    )
    def digest(byte: str) -> str:
        return f"blake3-256:{byte * 64}"

    packet = {
        "schemaId": "agent.semantic-protocols.agent-org-topology-overlay",
        "schemaVersion": "1",
        "sourceGenerationDigest": digest("1"),
        "baseTopologyGenerationDigest": digest("2"),
        "selector": "src/lib.rs#item=run",
        "agentIdentityDigest": digest("3"),
        "promptDigest": digest("4"),
        "summary": "run dispatches the request",
        "summaryDigest": digest("5"),
        "orgSource": "* Agent summary\n",
        "orgSourceDigest": digest("6"),
        "orgAstDigest": digest("7"),
        "overlayDigest": digest("8"),
        "relationships": [{
            "fromSelector": "src/lib.rs#item=run",
            "relation": "dispatches-to",
            "toSelector": "src/runtime.rs#item=dispatch",
        }],
        "terminal": {"state": "admitted", "terminalCount": 1, "reasonKind": None},
    }
    Draft202012Validator(schema).validate(packet)
