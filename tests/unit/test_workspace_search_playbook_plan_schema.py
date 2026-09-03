import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/workspace-search-playbook-plan.v1.schema.json").read_text()
)


def digest(value: str) -> str:
    return f"blake3-256:{value * 64}"


def plan() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-plan",
        "schemaVersion": "1",
        "projectId": "project-1",
        "workspaceId": "workspace-1",
        "contentGenerationDigest": digest("a"),
        "intent": "relationship",
        "query": "runtime owner graph",
        "normalizedTerms": ["graph", "owner", "runtime"],
        "languageConstraint": None,
        "maxConcurrency": 1,
        "routes": [
            {
                "languageId": "rust",
                "providerId": "asp-rust",
                "generationDigest": digest("a"),
                "extensions": [".rs"],
                "operation": "search",
                "normalizedTerms": ["graph", "owner", "runtime"],
                "selectors": [],
                "stages": [
                    {
                        "stage": "rg-acquisition",
                        "authority": "asp-server",
                        "policy": "content-required",
                        "dependsOn": [],
                    },
                    {
                        "stage": "provider-native-syntax",
                        "authority": "asp-rust",
                        "policy": "content-required",
                        "dependsOn": ["rg-acquisition"],
                    },
                    {
                        "stage": "tantivy-lexical",
                        "authority": "asp-server",
                        "policy": "accelerator-if-ready",
                        "dependsOn": ["provider-native-syntax"],
                    },
                    {
                        "stage": "python-graph",
                        "authority": "asp-python-graphs",
                        "policy": "intent-required",
                        "dependsOn": ["provider-native-syntax"],
                    },
                ],
                "budget": {
                    "maxResults": 32,
                    "maxGraphNodes": 128,
                    "deadlineMillis": 700,
                },
                "dependsOn": [],
                "publicCommand": [
                    "asp",
                    "rust",
                    "search",
                    "playbook",
                    "runtime owner graph",
                ],
            }
        ],
        "skippedLanguages": [],
        "work": {
            "filesystemReadCount": 0,
            "providerProcessCount": 0,
            "socketDiscoveryCount": 0,
        },
    }


def test_workspace_playbook_plan_v1_accepts_explicit_language_routes() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(plan())


def test_workspace_playbook_plan_v1_rejects_stage_reordering() -> None:
    receipt = plan()
    receipt["routes"][0]["stages"].reverse()
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(receipt)


def test_workspace_playbook_plan_v1_allows_all_languages_to_be_typed_skips() -> None:
    receipt = plan()
    receipt["routes"] = []
    receipt["skippedLanguages"] = [
        {"languageId": "rust", "reasonKind": "provider-unavailable"}
    ]
    jsonschema.Draft202012Validator(SCHEMA).validate(receipt)
