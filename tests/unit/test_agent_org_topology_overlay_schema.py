# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def digest(byte: str) -> str:
    return f"blake3-256:{byte * 64}"


def test_agent_org_topology_overlay_v1_contract_accepts_separate_identities() -> None:
    schema = json.loads(
        (ROOT / "schemas/agent-org-topology-overlay.v1.schema.json").read_text()
    )
    packet = {
        "schemaId": "agent.semantic-protocols.agent-org-topology-overlay",
        "schemaVersion": "1",
        "sourceGenerationDigest": digest("1"),
        "baseTopologyGenerationDigest": digest("2"),
        "selector": "src/lib.rs#item=run",
        "evidenceDigest": digest("9"),
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


def test_agent_org_topology_evidence_v1_keeps_exact_query_bytes_separate() -> None:
    schema = json.loads(
        (ROOT / "schemas/agent-org-topology-evidence.v1.schema.json").read_text()
    )
    packet = {
        "schemaId": "agent.semantic-protocols.agent-org-topology-evidence",
        "schemaVersion": "1",
        "caseId": "rust.bytes.core",
        "resourceId": "rust.bytes",
        "sourceGenerationDigest": digest("1"),
        "baseTopologyGenerationDigest": digest("2"),
        "selector": "rust://src/lib.rs#item/function/run",
        "sourceQueryOperationId": "query-source-1",
        "sourceBytesBase64": "cHViIGZuIHJ1bigpIHt9Cg==",
        "callableSkeletonOperationId": "query-skeleton-1",
        "callableSkeletonBytesBase64": "e30=",
        "evidenceDigest": digest("3"),
        "prompt": "Analyze only the exact evidence in this packet.",
        "promptDigest": digest("4"),
        "terminal": {"state": "ready", "terminalCount": 1, "reasonKind": None},
    }
    Draft202012Validator(schema).validate(packet)


def test_agent_org_topology_contribution_v1_requires_evidence_binding() -> None:
    schema = json.loads(
        (ROOT / "schemas/agent-org-topology-contribution.v1.schema.json").read_text()
    )
    packet = {
        "schemaId": "agent.semantic-protocols.agent-org-topology-contribution",
        "schemaVersion": "1",
        "caseId": "rust.bytes.core",
        "resourceId": "rust.bytes",
        "evidenceDigest": digest("1"),
        "agentIdentityDigest": digest("2"),
        "promptDigest": digest("3"),
        "summary": "run dispatches the admitted request",
        "orgSource": "* Agent summary\n",
        "relationships": [{
            "fromSelector": "rust://src/lib.rs#item/function/run",
            "relation": "dispatches-to",
            "toSelector": "rust://src/runtime.rs#item/function/dispatch",
        }],
    }
    Draft202012Validator(schema).validate(packet)
