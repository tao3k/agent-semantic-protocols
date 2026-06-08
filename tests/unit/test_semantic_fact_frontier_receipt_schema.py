"""Validate semantic fact frontier receipt schema."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "tests" / "unit"))

from schema_validation import schema_validator_for  # noqa: E402

_SCHEMA_PATH = _ROOT / "schemas" / "semantic-fact-frontier-receipt.v1.schema.json"
_FIXTURES_PATH = _ROOT / "schemas" / "semantic-fact-frontier-receipt.fixtures.v1.json"


def _validator() -> Draft202012Validator:
    return schema_validator_for(_SCHEMA_PATH)


def test_frontier_receipt_schema_accepts_followed_exact_selector() -> None:
    receipt = _receipt()

    errors = sorted(_validator().iter_errors(receipt), key=lambda error: list(error.path))

    assert not errors, [f"{list(error.path)}: {error.message}" for error in errors]


def test_frontier_receipt_real_project_fixtures_are_valid() -> None:
    catalog = _load_json(_FIXTURES_PATH)
    validator = _validator()

    assert catalog["schemaId"] == (
        "agent.semantic-protocols.semantic-fact-frontier-receipt.fixtures"
    )
    for fixture in catalog["fixtures"]:
        receipt = fixture["receipt"]
        errors = sorted(
            validator.iter_errors(receipt),
            key=lambda error: list(error.path),
        )
        assert not errors, [
            f"{fixture['fixtureId']} {list(error.path)}: {error.message}"
            for error in errors
        ]
        assert receipt["metrics"]["frontierReturnedCount"] == len(
            receipt["frontierReturned"]
        )
        assert receipt["metrics"]["frontierFollowedCount"] == len(
            receipt["frontierFollowed"]
        )
        reads_from_frontier = [
            read["fromFrontier"] for read in receipt["codeActuallyRead"]
        ]
        if fixture["fixtureId"].endswith("low-recall-runtime"):
            assert any(not from_frontier for from_frontier in reads_from_frontier)
        else:
            assert all(reads_from_frontier)


def test_frontier_receipt_schema_requires_code_read_accounting() -> None:
    receipt = _receipt()
    receipt.pop("codeActuallyRead")

    errors = sorted(_validator().iter_errors(receipt), key=lambda error: list(error.path))

    assert any("'codeActuallyRead' is a required property" in error.message for error in errors)


def test_frontier_receipt_schema_rejects_unclassified_read_kind() -> None:
    receipt = _receipt()
    receipt["codeActuallyRead"][0]["readKind"] = "wide-window"

    errors = sorted(_validator().iter_errors(receipt), key=lambda error: list(error.path))

    assert any("is not one of" in error.message for error in errors)


def _receipt() -> dict[str, Any]:
    source_range = {
        "path": "src/cache.rs",
        "startLine": 20,
        "endLine": 24,
    }
    frontier_item = {
        "nodeId": "field:cache.entries",
        "action": "code",
        "selector": "src/cache.rs:20:24",
        "owner": "src/cache.rs",
        "symbol": "entries",
        "range": source_range,
        "confidence": "exact",
        "freshness": "fresh",
    }
    return {
        "schemaId": "agent.semantic-protocols.semantic-fact-frontier-receipt",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-fact-frontier-feedback",
        "protocolVersion": "1",
        "receiptId": "rust.cache.entries.frontier-1",
        "receiptKind": "frontier",
        "taskFingerprint": "task:cache-writeback",
        "commandFingerprint": "command:graph-turbo:cache-writeback",
        "selector": "src/cache.rs:20:24",
        "owner": "src/cache.rs",
        "symbol": "entries",
        "range": source_range,
        "frontierReturned": [
            frontier_item,
            {
                "nodeId": "test:cache-writeback",
                "action": "tests",
                "selector": None,
                "owner": "tests/cache.rs",
                "symbol": "cache_writeback_roundtrip",
                "range": None,
                "confidence": "high",
                "freshness": "cache-hit",
            },
        ],
        "frontierFollowed": [frontier_item],
        "codeActuallyRead": [
            {
                "selector": "src/cache.rs:20:24",
                "owner": "src/cache.rs",
                "range": source_range,
                "readKind": "exact-selector",
                "fromFrontier": True,
            }
        ],
        "testCommand": {
            "argv": ["cargo", "test", "cache_writeback_roundtrip"],
            "workdir": ".",
            "fingerprint": "command:cargo-test:cache-writeback",
        },
        "testResult": {
            "status": "passed",
            "exitCode": 0,
            "summary": "cache writeback roundtrip passed",
        },
        "editTouchedOwner": ["src/cache.rs"],
        "outputFingerprint": "sha256:frontier-output",
        "metrics": {
            "frontierFollowRate": 0.5,
            "rawReadFallbackCount": 0,
        },
    }


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))
