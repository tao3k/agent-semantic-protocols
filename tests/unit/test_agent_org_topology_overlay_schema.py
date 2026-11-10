# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
import tomllib
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def digest(byte: str) -> str:
    return f"blake3-256:{byte * 64}"


def test_agent_org_topology_suite_executes_complex_scheme_for_every_corpus() -> None:
    suite_path = ROOT / "benchmarks/live-corpus-agent-org-topology-scenarios.v1.toml"
    suite = tomllib.loads(suite_path.read_text())
    baseline = tomllib.loads(
        (ROOT / "benchmarks/live-corpus-scheme-scenarios.v1.toml").read_text()
    )
    schema = json.loads(
        (ROOT / "schemas/agent-org-topology-scenario-suite.v1.schema.json").read_text()
    )
    Draft202012Validator(schema).validate(suite)
    cases = {case["case_id"]: case for case in suite["cases"]}
    baseline_cases = {case["case_id"]: case for case in baseline["cases"]}
    assert len(cases) == 17
    assert cases.keys() == baseline_cases.keys()
    for case_id, case in cases.items():
        assert case["resource_id"] == baseline_cases[case_id]["resource_id"]
        assert "(chain " in case["composed_search"]
        assert "(intersect " in case["composed_search"]
        assert case["composed_search"].count("(rg ") >= 2
        assert case["composed_search"].count("(tantivy ") >= 2
        assert case["minimum_composed_candidates"] >= 2
        assert case["multi_source_query"].count("{{selectors}}") == 1
        assert "(projection source)" in case["multi_source_query"]
        assert case["multi_callable_skeleton_query"].count("{{selectors}}") == 1
        assert "(projection callable-skeleton)" in case["multi_callable_skeleton_query"]


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
        "anchorSelector": "rust://src/lib.rs#item/function/run",
        "scopeSelectors": [
            "rust://src/lib.rs#item/function/run",
            "rust://src/runtime.rs#item/function/dispatch",
        ],
        "composedSearchOperationId": "search-composed-1",
        "composedSearchScheme": "(search (producers (language rust)) (chain (intersect (rg \"run\") (tantivy \"run\"))))",
        "composedSearchSchemeDigest": digest("5"),
        "sourceQueryOperationId": "query-source-1",
        "sourceMaterializations": [
            {"selector": "rust://src/lib.rs#item/function/run", "bytesBase64": "cHViIGZuIHJ1bigpIHt9Cg=="},
            {"selector": "rust://src/runtime.rs#item/function/dispatch", "bytesBase64": "cHViIGZuIGRpc3BhdGNoKCkge30K"},
        ],
        "callableSkeletonOperationId": "query-skeleton-1",
        "callableSkeletonMaterializations": [
            {"selector": "rust://src/lib.rs#item/function/run", "bytesBase64": "e30="},
            {"selector": "rust://src/runtime.rs#item/function/dispatch", "bytesBase64": "e30="},
        ],
        "evidenceDigest": digest("3"),
        "prompt": "Analyze only the exact evidence in this packet.",
        "promptDigest": digest("4"),
        "requiredRelationKinds": ["dispatches-to"],
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
