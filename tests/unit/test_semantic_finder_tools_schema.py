"""Validate the shared semantic finder tools schema."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = _REPO_ROOT / "schemas" / "semantic-finder-tools.v1.schema.json"


def _schema() -> dict:
    return json.loads(_SCHEMA_PATH.read_text(encoding="utf-8"))


def _valid_catalog() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.semantic-finder-tools",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": ".",
        "toolCatalog": [
            {
                "toolId": "rg",
                "displayName": "ripgrep",
                "roles": ["source-scan"],
                "boundary": "lexical-candidate",
                "inputFormats": ["line-list"],
                "outputFormats": ["rg-json", "rg-n"],
                "supports": {
                    "headless": True,
                    "interactive": False,
                    "json": True,
                    "fuzzy": False,
                    "regex": True,
                    "structural": False,
                    "rewrite": False,
                },
                "agentCallable": False,
            },
            {
                "toolId": "lexical",
                "displayName": "lexical",
                "roles": ["fuzzy-filter"],
                "boundary": "fuzzy-filter",
                "inputFormats": ["rg-n", "path-list", "owner-labels"],
                "outputFormats": ["line-list"],
                "supports": {
                    "headless": True,
                    "interactive": False,
                    "json": False,
                    "fuzzy": True,
                    "regex": False,
                    "structural": False,
                    "rewrite": False,
                },
                "agentCallable": False,
            },
            {
                "toolId": "ast-grep",
                "displayName": "ast-grep",
                "roles": ["structural-search"],
                "boundary": "structural-recipe",
                "inputFormats": ["line-list"],
                "outputFormats": ["structural-matches"],
                "supports": {
                    "headless": True,
                    "interactive": False,
                    "json": True,
                    "fuzzy": False,
                    "regex": True,
                    "structural": True,
                    "rewrite": True,
                },
                "agentCallable": False,
            },
        ],
        "pipelines": [
            {
                "pipelineId": "lexical-rg-lines",
                "surface": "search-lexical",
                "purpose": "fuzzy-lexical-candidates",
                "defaultFor": ["lexical"],
                "stages": [
                    {
                        "stageId": "rg-source-scan",
                        "toolId": "rg",
                        "role": "source-scan",
                        "mode": "regex",
                        "inputFormat": "line-list",
                        "outputFormat": "rg-n",
                        "headlessRequired": True,
                        "agentAuthoredArgsAllowed": False,
                    },
                    {
                        "stageId": "lexical-rank",
                        "toolId": "lexical",
                        "role": "fuzzy-filter",
                        "mode": "fuzzy",
                        "inputFormat": "rg-n",
                        "outputFormat": "line-list",
                        "headlessRequired": True,
                        "agentAuthoredArgsAllowed": False,
                    },
                ],
                "output": {
                    "candidateBasis": "source-lines",
                    "emitsRawSource": False,
                    "maxCandidates": 120,
                    "next": ["owner-grouping", "nearest-item", "tests"],
                },
            },
            {
                "pipelineId": "pattern-ast-grep-recipe",
                "surface": "search-pattern",
                "purpose": "structural-candidates",
                "defaultFor": ["pattern"],
                "stages": [
                    {
                        "stageId": "ast-grep-recipe",
                        "toolId": "ast-grep",
                        "role": "structural-search",
                        "mode": "recipe",
                        "inputFormat": "line-list",
                        "outputFormat": "structural-matches",
                        "headlessRequired": True,
                        "agentAuthoredArgsAllowed": False,
                    }
                ],
                "output": {
                    "candidateBasis": "structural-matches",
                    "emitsRawSource": False,
                    "maxCandidates": 80,
                    "next": ["owner-grouping", "nearest-item"],
                },
            },
        ],
        "provenanceSamples": [
            {
                "pipelineId": "lexical-rg-lines",
                "backend": "rg+lexical",
                "matchMode": "fuzzy",
                "candidateBasis": "source-lines",
                "sourceSearchPasses": 1,
                "inputCandidates": 420,
                "selectedCandidates": 24,
                "fuzzyFilter": {
                    "toolId": "lexical",
                    "query": "rawsearch",
                    "scoreBasis": "rank",
                },
                "stages": [
                    {
                        "stageId": "rg-source-scan",
                        "toolId": "rg",
                        "role": "source-scan",
                        "inputFormat": "line-list",
                        "outputFormat": "rg-n",
                        "candidatesIn": 0,
                        "candidatesOut": 420,
                        "elapsedMs": 12,
                    },
                    {
                        "stageId": "lexical-rank",
                        "toolId": "lexical",
                        "role": "fuzzy-filter",
                        "inputFormat": "rg-n",
                        "outputFormat": "line-list",
                        "candidatesIn": 420,
                        "candidatesOut": 24,
                        "elapsedMs": 4,
                    },
                ],
            }
        ],
    }


class SemanticFinderToolsSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = Draft202012Validator(_schema())

    def _assert_valid(self, document: dict) -> None:
        errors = sorted(self.validator.iter_errors(document), key=lambda error: list(error.path))
        self.assertEqual([], errors)

    def test_rg_lexical_and_ast_grep_catalog_validates(self) -> None:
        self._assert_valid(_valid_catalog())

    def test_lexical_stage_must_be_headless_provider_owned_filter(self) -> None:
        document = _valid_catalog()
        lexical_stage = document["pipelines"][0]["stages"][1]
        lexical_stage["agentAuthoredArgsAllowed"] = True

        errors = sorted(self.validator.iter_errors(document), key=lambda error: list(error.path))

        self.assertTrue(
            any("False was expected" in error.message for error in errors),
            [error.message for error in errors],
        )

    def test_ast_grep_is_structural_not_fuzzy_text(self) -> None:
        document = _valid_catalog()
        ast_grep_tool = document["toolCatalog"][2]
        ast_grep_tool["supports"]["fuzzy"] = True

        errors = sorted(self.validator.iter_errors(document), key=lambda error: list(error.path))

        self.assertTrue(
            any("False was expected" in error.message for error in errors),
            [error.message for error in errors],
        )

    def test_fd_exa_path_fallback_provenance_validates(self) -> None:
        document = copy.deepcopy(_valid_catalog())
        document["provenanceSamples"].append(
            {
                "pipelineId": "fd-exa-paths",
                "backend": "fd+exa",
                "matchMode": "literal",
                "candidateBasis": "paths",
                "sourceSearchPasses": 1,
                "inputCandidates": 2048,
                "selectedCandidates": 1,
                "stages": [
                    {
                        "stageId": "exa-path-list",
                        "toolId": "exa",
                        "role": "path-list",
                        "inputFormat": "path-list",
                        "outputFormat": "path-list",
                        "candidatesIn": 0,
                        "candidatesOut": 2048,
                        "elapsedMs": 7,
                        "fields": {
                            "fileListPasses": 1,
                            "queryTerms": ["fromexaruntime", "missingterm"],
                        },
                    }
                ],
                "fields": {
                    "fallbackFrom": "fd",
                    "queryTerms": ["fromexaruntime", "missingterm"],
                },
            }
        )

        self._assert_valid(document)

    def test_raw_command_fields_are_not_part_of_the_contract(self) -> None:
        document = copy.deepcopy(_valid_catalog())
        document["pipelines"][0]["stages"][0]["argv"] = ["rg", "--json", "needle", "."]

        errors = sorted(self.validator.iter_errors(document), key=lambda error: list(error.path))

        self.assertTrue(
            any("Additional properties are not allowed" in error.message for error in errors),
            [error.message for error in errors],
        )


if __name__ == "__main__":
    unittest.main()
