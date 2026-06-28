from __future__ import annotations

import json
from pathlib import Path

import jsonschema
import pytest

from tools.semantic_sandtable.graph_search_observation import (
    AbsolutePathError,
    assert_no_absolute_paths,
    observations_from_report,
    write_jsonl,
)


SCHEMA_PATH = Path("schemas/graph-search-observation.v1.schema.json")


def _schema() -> dict:
    return json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))


def test_live_token_receipt_becomes_graph_search_observation() -> None:
    report = {
        "runId": "deepseek-live-smoke",
        "scenarios": [
            {
                "id": "deepseek-token-receipt",
                "language": "rust",
                "status": "pass",
                "steps": [
                    {
                        "command": ["asp", "rust", "search", "pipe", "token"],
                        "observations": {
                            "tokenCost": {
                                "inputTokens": 8,
                                "outputTokens": 1,
                                "totalTokens": 9,
                                "providers": ["deepseek"],
                                "models": ["deepseek-chat"],
                                "source": "openai-compatible-live",
                            }
                        },
                    }
                ],
            }
        ],
    }

    observations = observations_from_report(report, source_ref="sandtable/deepseek-live-smoke.json")

    assert len(observations) == 1
    observation = observations[0]
    jsonschema.Draft202012Validator(_schema()).validate(observation)
    assert observation["cost"]["inputTokens"] == 8
    assert observation["cost"]["outputTokens"] == 1
    assert observation["cost"]["totalTokens"] == 9
    assert observation["subject"]["provider"] == "deepseek"
    assert observation["subject"]["model"] == "deepseek-chat"
    assert observation["providerHealth"][0]["status"] == "ok"
    assert_no_absolute_paths(observation)


def test_provider_runtime_failure_is_recorded_without_host_paths() -> None:
    report = {
        "scenarios": [
            {
                "id": "julia-provider-health",
                "language": "julia",
                "status": "fail",
                "workdir": "/workspace/private/repo",
                "steps": [
                    {
                        "command": [
                            "/workspace/private/.local/bin/asp-julia-harness",
                            "query",
                            "--selector",
                            "analyzers/AspGraphsSearch.jl",
                        ],
                        "exitCode": 127,
                        "stderr": "Library not loaded: @rpath/libjulia.1.12.dylib referenced from /workspace/private/.local/bin/asp-julia-harness",
                    }
                ],
            }
        ],
    }

    observation = observations_from_report(report)[0]

    assert observation["providerHealth"][0]["status"] == "failed"
    assert observation["providerHealth"][0]["failureKind"] == "dynamic-library-rpath"
    assert observation["providerHealth"][0]["binaryRef"] == {
        "kind": "home-local-bin",
        "value": "asp-julia-harness",
    }
    assert "/workspace/private" not in json.dumps(observation)
    assert_no_absolute_paths(observation)


def test_absolute_path_contract_rejects_github_unsafe_values() -> None:
    with pytest.raises(AbsolutePathError):
        assert_no_absolute_paths({"fixture": "/workspace/private/repo"})


def test_schema_rejects_absolute_path_refs() -> None:
    observation = observations_from_report(
        {
            "scenarios": [
                {
                    "id": "absolute-path-schema-check",
                    "language": "rust",
                    "status": "pass",
                    "steps": [],
                }
            ]
        },
        source_ref="sandtable/report.json",
    )[0]
    observation["source"]["pathRef"]["value"] = "/workspace/private/report.json"

    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(_schema()).validate(observation)


def test_jsonl_writer_preserves_safe_observation(tmp_path: Path) -> None:
    report = {
        "scenarios": [
            {
                "id": "python-query-route",
                "language": "python",
                "status": "pass",
                "graphEvidence": {
                    "owners": ["packages/python/tools/src/tools/semantic_sandtable/agent_observation_tokens.py"],
                    "items": [{"name": "token_cost_from_messages", "kind": "function"}],
                },
                "steps": [
                    {
                        "command": ["asp", "python", "search", "owner", "src/tools/semantic_sandtable/agent_observation_tokens.py"],
                        "observations": {
                            "candidateCount": 3,
                            "selectedCount": 1,
                        },
                    }
                ],
            }
        ]
    }
    output = tmp_path / "gso.jsonl"

    write_jsonl(observations_from_report(report), output)

    lines = output.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    observation = json.loads(lines[0])
    jsonschema.Draft202012Validator(_schema()).validate(observation)
    assert observation["graphEvidence"]["owners"][0]["value"].endswith("agent_observation_tokens.py")
    assert observation["cost"]["candidateCount"] == 3
    assert observation["cost"]["selectedCount"] == 1


def test_typescript_route_observation_uses_same_language_neutral_contract() -> None:
    report = {
        "scenarios": [
            {
                "id": "typescript-owner-frontier",
                "language": "typescript",
                "status": "pass",
                "evidenceState": {
                    "knownOwner": True,
                    "knownSelector": False,
                    "queryQuality": "high",
                    "packageCohesion": "cohesive",
                },
                "routeDecision": {
                    "chosen": "owner-items",
                    "recommendedNext": "asp typescript search owner src/router.ts items",
                    "nextCommandKind": "owner-items",
                    "avoided": ["line-selector-as-action"],
                },
                "graphEvidence": {
                    "owners": ["src/router.ts"],
                    "items": [{"name": "routeSearchIntent", "kind": "function", "owner": "src/router.ts"}],
                    "edges": [{"from": "routeSearchIntent", "to": "renderFrontier", "relation": "calls"}],
                },
                "steps": [
                    {
                        "command": ["asp", "typescript", "search", "owner", "src/router.ts", "items"],
                        "observations": {"candidateCount": 5, "selectedCount": 2},
                    }
                ],
            }
        ]
    }

    observation = observations_from_report(report)[0]

    jsonschema.Draft202012Validator(_schema()).validate(observation)
    assert observation["subject"]["language"] == "typescript"
    assert observation["routeDecision"]["chosen"] == "owner-items"
    assert observation["cost"]["candidateCount"] == 5
    assert_no_absolute_paths(observation)
