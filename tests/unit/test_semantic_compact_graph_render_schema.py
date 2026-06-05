"""Validate the shared compact graph render schema."""

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def _reasoning_profile_contracts() -> list[object]:
    schema_path = (
        _PROTOCOL_REPO_ROOT / "schemas" / "semantic-compact-graph-render.v1.schema.json"
    )
    with schema_path.open("r", encoding="utf-8") as handle:
        schema = json.load(handle)
    return schema["properties"]["reasoningProfileContracts"]["const"]


def minimal_render_template() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-compact-graph-render",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "renderKind": "compact-graph-render-template",
        "rendererOwner": {
            "crate": "agent-semantic-protocol",
            "library": "agent_semantic_protocol::graph",
            "cli": "asp graph render --packet <path-or-> --view seeds",
            "inputPacketSchemaId": "agent.semantic-protocols.semantic-search-packet",
            "migrationAdapterAllowed": True,
            "providerIntegration": "shell-out",
            "providerLibraryDependencyAllowed": False,
            "providerLocalRendererAllowed": False,
            "targetHotPath": "agent-semantic-client-cache-query-render",
        },
        "sourcePacketSchemaId": "agent.semantic-protocols.semantic-search-packet",
        "viewHeaderContract": {
            "appliesWhen": "search --view seeds",
            "headerPrefix": "[search-<view>]",
            "headerIsGraphPacket": True,
            "graphBlockRequired": True,
            "legacySeedRowsAllowed": False,
            "legacySynthesisRowsAllowed": False,
        },
        "lineGrammar": {
            "header": "[search-<view>] (q=<query>|owner=<path>|root=<root>) [querySet=<n>|terms=<n>] [selector=<selector>] [scope=<scope>] [view=<view>] alg=<algorithm>",
            "microLegend": "legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next",
            "legend": "alias: graph:{<ID>=<nodeType>,...}; every rendered ID is declared",
            "alias": "<ID>=<nodeType>:<targetRole>(<target>)[@<locator>][!<action>]",
            "edge": "<SRC>{<DST>:<relation>,...}",
            "groupedEdge": "(<SRC>,...)><DST>:<relation>",
            "rankFrontier": "rank=<ID>,... frontier=<ID>.<action>,...",
            "entries": "entries=<profile>(<ID>,...=><return>+...),...",
            "entryAliasContract": "entries selector IDs must resolve to alias: graph declarations; known profile selectors must match the profile node-kind catalog",
            "omit": "omit=<omitted-semantic>[,<omitted-semantic>]",
            "avoid": "avoid=<anti-action>[,<anti-action>]",
            "denseAliasSeparator": ";",
        },
        "locatorPolicy": {
            "ownerItemCodeFrontier": "required",
            "sameOwnerLocator": "@<start>:<end>",
            "crossOwnerLocator": "@<path>:<start>:<end>",
        },
        "searchRoot": {
            "aliasPrefix": "G",
            "nodeType": "search",
            "declaredBy": ["legend", "edgeSource"],
            "standaloneAliasLine": False,
            "rankEligible": False,
            "frontierEligible": False,
        },
        "reasoningProfileContracts": _reasoning_profile_contracts(),
        "headerFields": {
            "identity": ["q", "root"],
            "algorithm": "alg",
            "retained": ["querySet", "terms", "selector", "scope", "view"],
        },
        "actionSpecs": [
            {
                "sourceKind": "owner",
                "nodeType": "owner",
                "targetRole": "path",
                "aliasPrefix": "O",
                "action": "owner",
            },
            {
                "sourceKind": "tests",
                "nodeType": "test",
                "targetRole": "path",
                "aliasPrefix": "T",
                "action": "tests",
            },
            {
                "sourceKind": "dependency",
                "nodeType": "dependency",
                "targetRole": "pkg",
                "aliasPrefix": "D",
                "action": "deps",
            },
            {
                "sourceKind": "finding",
                "nodeType": "finding",
                "targetRole": "finding",
                "aliasPrefix": "F",
                "action": "finding",
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

    def test_entries_line_declares_profile_selectors_and_returns(self) -> None:
        template = minimal_render_template()

        self.assertEqual(
            "entries=<profile>(<ID>,...=><return>+...),...",
            template["lineGrammar"]["entries"],
        )
        self.assertEqual(
            "entries selector IDs must resolve to alias: graph declarations; known profile selectors must match the profile node-kind catalog",
            template["lineGrammar"]["entryAliasContract"],
        )

    def test_omit_and_avoid_lines_are_schema_owned(self) -> None:
        template = minimal_render_template()

        self.assertEqual(
            "omit=<omitted-semantic>[,<omitted-semantic>]",
            template["lineGrammar"]["omit"],
        )
        self.assertEqual(
            "avoid=<anti-action>[,<anti-action>]",
            template["lineGrammar"]["avoid"],
        )

    def test_view_header_is_the_graph_packet_header(self) -> None:
        template = minimal_render_template()

        self.assertEqual(
            "[search-<view>]", template["viewHeaderContract"]["headerPrefix"]
        )
        self.assertTrue(template["viewHeaderContract"]["headerIsGraphPacket"])
        self.assertTrue(template["viewHeaderContract"]["graphBlockRequired"])
        self.assertFalse(template["viewHeaderContract"]["legacySeedRowsAllowed"])
        self.assertFalse(template["viewHeaderContract"]["legacySynthesisRowsAllowed"])

    def test_view_header_contract_rejects_optional_graph_blocks(self) -> None:
        template = minimal_render_template()
        template["viewHeaderContract"] = copy.deepcopy(template["viewHeaderContract"])
        template["viewHeaderContract"]["graphBlockRequired"] = False

        errors = self.validation_errors(template)

        self.assertTrue(any("True was expected" in message for message in errors))

    def test_renderer_owner_is_shared_protocol_crate(self) -> None:
        template = minimal_render_template()

        self.assertEqual("agent-semantic-protocol", template["rendererOwner"]["crate"])
        self.assertEqual(
            "agent_semantic_protocol::graph",
            template["rendererOwner"]["library"],
        )
        self.assertEqual("shell-out", template["rendererOwner"]["providerIntegration"])
        self.assertFalse(template["rendererOwner"]["providerLibraryDependencyAllowed"])
        self.assertFalse(template["rendererOwner"]["providerLocalRendererAllowed"])
        self.assertEqual(
            "agent-semantic-client-cache-query-render",
            template["rendererOwner"]["targetHotPath"],
        )

    def test_provider_local_renderer_is_not_the_contract_owner(self) -> None:
        template = minimal_render_template()
        template["rendererOwner"] = copy.deepcopy(template["rendererOwner"])
        template["rendererOwner"]["providerLocalRendererAllowed"] = True

        errors = self.validation_errors(template)

        self.assertTrue(any("False was expected" in message for message in errors))

    def test_provider_renderer_dependency_is_not_allowed(self) -> None:
        template = minimal_render_template()
        template["rendererOwner"] = copy.deepcopy(template["rendererOwner"])
        template["rendererOwner"]["providerLibraryDependencyAllowed"] = True

        errors = self.validation_errors(template)

        self.assertTrue(any("False was expected" in message for message in errors))

    def test_provider_integration_is_shell_out(self) -> None:
        template = minimal_render_template()
        template["rendererOwner"] = copy.deepcopy(template["rendererOwner"])
        template["rendererOwner"]["providerIntegration"] = "library"

        errors = self.validation_errors(template)

        self.assertTrue(
            any("'shell-out' was expected" in message for message in errors)
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

    def test_unknown_target_role_is_rejected(self) -> None:
        template = minimal_render_template()
        template["actionSpecs"] = copy.deepcopy(template["actionSpecs"])
        template["actionSpecs"][0]["targetRole"] = "filename"

        errors = self.validation_errors(template)

        self.assertTrue(
            any("'filename' is not one of" in message for message in errors)
        )

    def test_search_root_is_not_a_standalone_alias_line(self) -> None:
        template = minimal_render_template()
        template["searchRoot"] = copy.deepcopy(template["searchRoot"])
        template["searchRoot"]["standaloneAliasLine"] = True

        errors = self.validation_errors(template)

        self.assertTrue(any("False was expected" in message for message in errors))


if __name__ == "__main__":
    unittest.main()
