"""Validate schema-owned compact graph reasoning profile contracts."""

from __future__ import annotations

import json
from pathlib import Path
import unittest


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


def _reasoning_profile_contracts() -> dict[str, dict[str, object]]:
    schema_path = (
        _PROTOCOL_REPO_ROOT / "schemas" / "semantic-compact-graph-render.v1.schema.json"
    )
    with schema_path.open("r", encoding="utf-8") as handle:
        schema = json.load(handle)
    return {
        contract["profile"]: contract
        for contract in schema["properties"]["reasoningProfileContracts"]["const"]
    }


class CompactGraphReasoningProfileContractTests(unittest.TestCase):
    def test_owner_tests_has_only_owner_selector(self) -> None:
        contracts = _reasoning_profile_contracts()

        self.assertEqual(
            [{"kind": "owner", "required": True}],
            contracts["owner-tests"]["selectors"],
        )

    def test_finding_frontier_owner_selector_is_optional(self) -> None:
        contracts = _reasoning_profile_contracts()

        self.assertEqual(
            [
                {"kind": "finding", "required": True},
                {"kind": "owner", "required": False},
            ],
            contracts["finding-frontier"]["selectors"],
        )

    def test_feature_cfg_returns_are_fixed_entries(self) -> None:
        contracts = _reasoning_profile_contracts()

        self.assertEqual(
            ["cfg-gates", "owners", "verification-surfaces"],
            contracts["feature-cfg"]["returns"],
        )
