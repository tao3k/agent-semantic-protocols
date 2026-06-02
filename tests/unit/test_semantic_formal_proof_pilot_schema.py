from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def load_schema() -> dict:
    return json.loads(
        (ROOT / "schemas" / "semantic-formal-proof-pilot.v1.schema.json").read_text(
            encoding="utf-8"
        )
    )


def test_formal_proof_pilot_schema_accepts_dependency_graph_pilot() -> None:
    schema = load_schema()
    validator = Draft202012Validator(schema)

    validator.validate(
        {
            "schemaId": "agent.semantic-protocols.semantic-formal-proof-pilot",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.formal-proof-pilot",
            "protocolVersion": "1",
            "proofId": "rust.proof.dependency-graph-acyclicity",
            "producer": {
                "languageId": "rust",
                "providerId": "rs-harness",
                "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
            },
            "target": {
                "kind": "dependency-graph-acyclicity",
                "name": "owner dependency graph cycle detection",
                "ruleIds": ["AGENT-R009"],
                "ownerPath": "src/rules/agent_policy/dependency_graph.rs",
                "symbol": "owner_dependency_cycle_indices",
            },
            "method": {
                "kind": "exhaustive-small-model",
                "tool": "rs-harness",
                "command": [
                    "rs-harness",
                    "proof",
                    "pilot",
                    "dependency-graph-acyclicity",
                    "--max-nodes",
                    "4",
                    "--json",
                ],
            },
            "status": "proved-bounded",
            "claims": [
                {
                    "claimId": "cycle-detection-iff-directed-cycle",
                    "statement": "For all directed graphs up to four nodes, the rule core reports a cycle iff the graph contains a directed cycle.",
                    "status": "proved-bounded",
                }
            ],
            "checks": [
                {
                    "checkId": "exhaustive-directed-graphs-up-to-4",
                    "status": "proved-bounded",
                    "summary": "Checked all directed graphs with up to four nodes.",
                    "modelsChecked": 4166,
                    "maxNodes": 4,
                }
            ],
        }
    )
