"""Validate demo builders and semantic-search-packet schema projection."""

from __future__ import annotations

import unittest

from tools.lean_axle_proof_demo_artifacts import (
    build_obligation,
    build_receipt,
    build_recipe,
    build_report,
)
from tools.lean_axle_proof_demo_io import load_proof_artifacts
from tools.semantic_search_packet_fixture_projection import (
    build_semantic_search_packet_fixture_projection,
)
from tools.semantic_search_packet_projection import build_semantic_search_packet_projection

from .support import (
    REPO_ROOT,
    load_json,
    load_validator,
    packet_projection,
    schema_projection,
    validation_errors,
)


class SemanticProofDemoProjectionTests(unittest.TestCase):
    def test_demo_builders_emit_schema_valid_artifacts(self) -> None:
        formal, candidate = load_proof_artifacts()
        obligation = build_obligation("2026-07-02T00:00:00+00:00")
        recipe = build_recipe(
            "lean-4.31.0",
            30,
            "formal-statement.lean",
            "candidate-proof.lean",
            formal,
            candidate,
        )
        response_json = {"okay": True, "failed_declarations": [], "timings": {"total_ms": 1}}
        receipt = build_receipt(
            obligation,
            recipe,
            "lean-4.31.0",
            formal,
            candidate,
            response_json,
            schema_projection(),
            packet_projection(),
        )
        report = build_report(receipt)

        cases = [
            ("semantic-proof-obligation.v1.schema.json", obligation),
            ("semantic-proof-recipe.v1.schema.json", recipe),
            ("semantic-proof-receipt.v1.schema.json", receipt),
            ("semantic-formal-verification-report.v1.schema.json", report),
        ]
        for schema_name, payload in cases:
            with self.subTest(schema_name=schema_name):
                self.assertEqual(validation_errors(load_validator(schema_name), payload), [])

    def test_search_packet_schema_projection_extracts_selector_identity_facts(self) -> None:
        projection = build_semantic_search_packet_projection(
            REPO_ROOT / "schemas" / "semantic-search-packet.v1.schema.json"
        )
        fact_ids = {fact["id"] for fact in projection.facts}

        self.assertIn("item.structuralSelector", fact_ids)
        self.assertIn("routeAction.displayLineRange", fact_ids)
        self.assertIn("windowSetTarget.sourceLocatorHint", fact_ids)
        self.assertIn("semantic_search_packet_schema_projects_selector_identity", projection.candidate_lean)

    def test_search_packet_fixture_projection_classifies_identity_contract(self) -> None:
        fixture_dir = REPO_ROOT / "tests" / "fixtures" / "semantic_search_packet"
        validator = load_validator("semantic-search-packet.v1.schema.json")

        bad_packet = load_json(fixture_dir / "bad_path_line_identity_packet.json")
        good_packet = load_json(fixture_dir / "good_selector_identity_packet.json")
        self.assertEqual(validation_errors(validator, bad_packet), [])
        self.assertEqual(validation_errors(validator, good_packet), [])

        bad_projection = build_semantic_search_packet_fixture_projection(
            fixture_dir / "bad_path_line_identity_packet.json"
        )
        good_projection = build_semantic_search_packet_fixture_projection(
            fixture_dir / "good_selector_identity_packet.json"
        )
        provider_projection = build_semantic_search_packet_fixture_projection(
            fixture_dir / "good_selector_identity_packet.json",
            source_kind="provider-output",
        )

        self.assertEqual(bad_projection.source_kind, "contract-fixture")
        self.assertEqual(bad_projection.identity_kind, "path-line-only")
        self.assertFalse(bad_projection.contract_valid)
        self.assertIn("rejects_path_line_fallback", bad_projection.candidate_lean)

        self.assertEqual(good_projection.identity_kind, "selector-owned")
        self.assertTrue(good_projection.contract_valid)
        self.assertIn("accepts_selector_identity", good_projection.candidate_lean)
        self.assertEqual(provider_projection.source_kind, "provider-output")
