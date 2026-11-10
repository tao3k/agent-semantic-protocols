# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema
from referencing import Registry, Resource


ROOT = Path(__file__).parents[2]
SCHEMA_ROOT = ROOT / "schemas"


def digest(character: str) -> str:
    return f"blake3-256:{character * 64}"


def identity() -> dict[str, str]:
    return {
        "projectId": "project",
        "workspaceId": "workspace",
        "sourceRootDigest": digest("a"),
        "providerDigest": digest("b"),
        "schemaDigest": digest("c"),
        "generationCandidateDigest": digest("d"),
    }


def schema(name: str) -> dict:
    return json.loads((SCHEMA_ROOT / name).read_text())


def test_cold_rg_pathspec_is_content_generation_bound() -> None:
    jsonschema.Draft202012Validator(
        schema("cold-rg-corpus-receipt.v1.schema.json")
    ).validate(
        {
            "schemaId": "agent.semantic-protocols.cold-rg-corpus-receipt",
            "schemaVersion": "1",
            "contentGenerationDigest": digest("a"),
            "ownerCount": 4096,
            "corpusDigest": digest("b"),
            "ownerSpansDigest": digest("c"),
        }
    )


def test_accelerator_receipt_requires_equivalence_corpus() -> None:
    definitions = schema("semantic-search-definitions.v1.schema.json")
    accelerator = schema("lexical-accelerator-receipt.v1.schema.json")
    registry = Registry().with_resource(
        definitions["$id"], Resource.from_contents(definitions)
    )
    jsonschema.Draft202012Validator(accelerator, registry=registry).validate(
        {
            "schemaId": "agent.semantic-protocols.lexical-accelerator-receipt",
            "schemaVersion": "1",
            "identity": identity(),
            "contentGenerationDigest": digest("1"),
            "lexicalPlanDigest": digest("2"),
            "tantivyArtifactDigest": digest("3"),
            "equivalenceCases": [
                {
                    "normalizedQueryDigest": digest("4"),
                    "coldRgCandidateSetDigest": digest("5"),
                    "tantivyCandidateSetDigest": digest("5"),
                }
            ],
            "equivalenceDigest": digest("6"),
            "complete": True,
        }
    )
