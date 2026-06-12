"""Validate fzf search packet schema coverage."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = _REPO_ROOT / "schemas" / "semantic-search-packet.v1.schema.json"


class SemanticSearchPacketFzfSchemaTests(unittest.TestCase):
    def test_fzf_view_and_synthesis_scope_are_valid(self) -> None:
        schema = json.loads(_SCHEMA_PATH.read_text(encoding="utf-8"))
        validator = Draft202012Validator(schema)
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-search-packet",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "rust",
            "providerId": "rs-harness",
            "binary": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
            "method": "search/fzf",
            "projectRoot": ".",
            "view": "fzf",
            "renderMode": "seeds",
            "query": "rawsearch",
            "header": {
                "kind": "search-fzf",
                "fields": {
                    "backend": "rg+fzf",
                    "matchMode": "fuzzy",
                    "candidateBasis": "source-lines",
                },
            },
            "nodes": [],
            "edges": [],
            "owners": [],
            "hits": [],
            "findings": [],
            "nextActions": [],
            "notes": [],
            "searchSynthesis": {
                "algorithm": "fzf-owner-frontier",
                "scope": "fzf",
                "summary": "fzf candidates normalized into owner seeds",
                "selectedOwners": 0,
            },
            "runtimeCost": {
                "cacheStatus": "cold",
                "elapsedMs": 8,
                "fields": {
                    "finderPipelineId": "fzf-rg-lines",
                    "sourceSearchPasses": 1,
                    "inputCandidates": 120,
                    "selectedCandidates": 12,
                },
            },
        }

        errors = sorted(validator.iter_errors(packet), key=lambda error: list(error.path))

        self.assertEqual([], errors)

    def test_finder_no_output_receipt_is_valid(self) -> None:
        schema = json.loads(_SCHEMA_PATH.read_text(encoding="utf-8"))
        validator = Draft202012Validator(schema)
        packet = {
            "schemaId": "agent.semantic-protocols.semantic-search-packet",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "languageId": "rust",
            "providerId": "rs-harness",
            "binary": "rs-harness",
            "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
            "method": "search/fzf",
            "projectRoot": ".",
            "view": "fzf",
            "renderMode": "seeds",
            "query": "missing-owner",
            "header": {
                "kind": "search-fzf",
                "fields": {
                    "backend": "fd",
                    "candidateBasis": "paths",
                },
            },
            "nodes": [],
            "edges": [],
            "owners": [],
            "hits": [],
            "findings": [],
            "nextActions": [],
            "notes": [],
            "noOutput": {
                "reason": "no-candidates",
                "sourceTrace": [
                    {
                        "source": "finder",
                        "status": "empty",
                        "candidateCount": 0,
                        "fields": {
                            "backend": "fd",
                            "candidateBasis": "paths",
                        },
                    }
                ],
                "nextActions": [
                    {
                        "kind": "rg-query",
                        "target": "missing-owner",
                        "fields": {
                            "command": "asp rg -query 'missing-owner' '.'",
                        },
                    }
                ],
                "avoidNextActions": [
                    {
                        "kind": "repeat-flat-fd",
                        "target": "missing-owner",
                        "reason": "finder returned no candidates",
                    }
                ],
            },
        }

        errors = sorted(validator.iter_errors(packet), key=lambda error: list(error.path))

        self.assertEqual([], errors)


if __name__ == "__main__":
    unittest.main()
