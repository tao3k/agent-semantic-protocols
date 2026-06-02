from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def load_schema(name: str) -> dict:
    return json.loads((ROOT / "schemas" / name).read_text(encoding="utf-8"))


def test_invariant_candidate_schema_accepts_p0_catalog() -> None:
    schema = load_schema("semantic-invariant-candidate.v1.schema.json")
    validator = Draft202012Validator(schema)

    validator.validate(
        {
            "schemaId": "agent.semantic-protocols.semantic-invariant-candidate",
            "schemaVersion": "1",
            "candidates": [
                {
                    "invariantId": "agent-r027.src.model.rs:42",
                    "sourceRuleId": "AGENT-R027",
                    "rulePackId": "rust.agent_policy",
                    "kind": "primitive-type-alias-boundary",
                    "status": "candidate",
                    "severity": "info",
                    "title": "Public semantic type alias uses primitive carrier",
                    "hypothesis": "A public semantic alias over a primitive carrier should be promoted to a named boundary.",
                    "location": {"path": "src/model.rs", "line": 42, "column": 0},
                    "evidence": [
                        {
                            "kind": "finding",
                            "summary": "AGENT-R027 raised from parser-owned public type alias facts.",
                            "location": {"path": "src/model.rs", "line": 42},
                            "fields": {"requirement": "replace this alias with a newtype"},
                        }
                    ],
                    "requiredReceipts": ["cargo-check", "cargo-test", "clippy"],
                    "proofTargets": ["public-api-shape"],
                }
            ],
        }
    )


def test_search_packet_exposes_invariant_candidates_as_mergeable_surface() -> None:
    schema = load_schema("semantic-search-packet.v1.schema.json")

    assert (
        schema["properties"]["invariantCandidates"]["items"]["$ref"]
        == "semantic-invariant-candidate.v1.schema.json#/$defs/invariantCandidate"
    )
    merge_values = schema["$defs"]["queryComposition"]["properties"]["merge"]["items"][
        "enum"
    ]
    assert "invariantCandidates" in merge_values
