"""Validate the shared compact graph render schema."""

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_render_template() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-compact-graph-render",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "renderKind": "compact-graph-render-template",
        "sourcePacketSchemaId": "agent.semantic-protocols.semantic-search-packet",
        "lineGrammar": {
            "header": "[search-<view>] (q=<query>|root=<root>) [querySet=<n>] [selector=<selector>] [scope=<scope>] alg=<algorithm>",
            "legend": "alias: graph:{<ID>=<nodeType>,...}",
            "alias": "<ID>=<nodeType>:<target>!<action>",
            "edge": "<SRC>{<DST>:<relation>,...}",
            "groupedEdge": "(<SRC>,...)><DST>:<relation>",
            "rankFrontier": "rank=<ID>,... frontier=<ID>.<action>,...",
            "denseAliasSeparator": ";",
        },
        "headerFields": {
            "identity": ["q", "root"],
            "algorithm": "alg",
            "retained": ["querySet", "selector", "scope"],
        },
        "actionSpecs": [
            {
                "sourceKind": "search-root",
                "nodeType": "search",
                "aliasPrefix": "G",
                "action": "query",
            },
            {
                "sourceKind": "owner",
                "nodeType": "owner",
                "aliasPrefix": "O",
                "action": "owner",
            },
            {
                "sourceKind": "tests",
                "nodeType": "test",
                "aliasPrefix": "T",
                "action": "tests",
            },
            {
                "sourceKind": "dependency",
                "nodeType": "dependency",
                "aliasPrefix": "D",
                "action": "deps",
            },
        ],
        "relationSpecs": [
            {"targetNodeType": "owner", "relation": "selects"},
            {"targetNodeType": "test", "relation": "covers"},
            {"targetNodeType": "query", "relation": "matches"},
            {"targetNodeType": "dependency", "relation": "uses"},
            {"targetNodeType": "import", "relation": "imports"},
            {"targetNodeType": "symbol", "relation": "contains"},
            {"targetNodeType": "item", "relation": "contains"},
            {"targetNodeType": "doc", "relation": "explains"},
            {"targetNodeType": "finding", "relation": "flags"},
            {"targetNodeType": "feature", "relation": "gates"},
            {"targetNodeType": "cfg", "relation": "gates"},
        ],
    }


class SemanticCompactGraphRenderSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT
            / "schemas"
            / "semantic-compact-graph-render.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.schema = json.load(handle)
        self.validator = Draft202012Validator(self.schema)

    def validation_errors(self, template: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(template)]

    def test_minimal_render_template_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(minimal_render_template()))

    def test_rank_and_frontier_share_one_compact_line(self) -> None:
        template = minimal_render_template()

        self.assertEqual(
            "rank=<ID>,... frontier=<ID>.<action>,...",
            template["lineGrammar"]["rankFrontier"],
        )

    def test_unknown_action_is_rejected(self) -> None:
        template = minimal_render_template()
        template["actionSpecs"] = copy.deepcopy(template["actionSpecs"])
        template["actionSpecs"][0]["action"] = "open"

        errors = self.validation_errors(template)

        self.assertTrue(any("'open' is not one of" in message for message in errors))

    def test_alias_prefix_is_single_uppercase_letter(self) -> None:
        template = minimal_render_template()
        template["actionSpecs"] = copy.deepcopy(template["actionSpecs"])
        template["actionSpecs"][0]["aliasPrefix"] = "owner"

        errors = self.validation_errors(template)

        self.assertTrue(any("does not match" in message for message in errors))

    def test_unknown_relation_is_rejected(self) -> None:
        template = minimal_render_template()
        template["relationSpecs"] = copy.deepcopy(template["relationSpecs"])
        template["relationSpecs"][0]["relation"] = "maybe"

        errors = self.validation_errors(template)

        self.assertTrue(any("'maybe' is not one of" in message for message in errors))


if __name__ == "__main__":
    unittest.main()
