# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the semantic formal proof pilot schema contract."""

from __future__ import annotations

from pathlib import Path

from unit.schema_validation import schema_validator_for


_ROOT = Path(__file__).resolve().parents[2]


def _schema_path() -> Path:
    return _ROOT / "schemas" / "semantic-formal-proof-pilot.v1.schema.json"


def test_formal_proof_pilot_schema_accepts_dependency_graph_pilot() -> None:
    validator = schema_validator_for(_schema_path())

    validator.validate(
        {
            "schemaId": "agent.semantic-protocols.semantic-formal-proof-pilot",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.formal-proof-pilot",
            "protocolVersion": "1",
            "proofId": "rust.proof.dependency-graph-acyclicity",
            "producer": {
                "languageId": "rust",
                "providerId": "asp-rust",
                "namespace": "agent.semantic-protocols.languages.rust.asp-rust",
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
                "tool": "asp-rust",
                "command": [
                    "asp-rust",
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
