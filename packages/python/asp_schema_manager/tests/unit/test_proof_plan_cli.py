"""Schema Manager proof-plan CLI projection tests."""

from __future__ import annotations

import io
import json
from typing import Any

from asp_schema_manager import cli
from asp_schema_manager import _proof_plan_cli as proof_plan_cli


def _plan(state: str = "current") -> dict:
    return {
        "projectionKind": "schema-logical-proof-plan",
        "projectionVersion": "1",
        "planDigest": "sha256:" + "a" * 64,
        "validity": {"state": state},
        "schemas": [],
    }


def test_proof_plan_passes_selected_schemas_and_renders_json(monkeypatch) -> None:
    observed = {}

    def generate(*args: object, **kwargs: Any) -> dict:
        observed.update(kwargs)
        return _plan()

    monkeypatch.setattr(proof_plan_cli, "generate_proof_plan", generate)
    output = io.StringIO()
    status = cli.main(
        [
            "proof-plan",
            "--schema",
            "schemas/a.v1.schema.json",
            "--schema",
            "schemas/b.v1.schema.json",
            "--json",
        ],
        stdout=output,
    )

    assert status == 0
    assert observed["schema_paths"] == [
        "schemas/a.v1.schema.json",
        "schemas/b.v1.schema.json",
    ]
    assert (
        json.loads(output.getvalue())["projectionKind"] == "schema-logical-proof-plan"
    )


def test_stale_expected_digest_returns_nonzero(monkeypatch) -> None:
    monkeypatch.setattr(
        proof_plan_cli,
        "generate_proof_plan",
        lambda *args, **kwargs: _plan("stale"),
    )

    status = cli.main(
        ["proof-plan", "--expected-plan-digest", "sha256:" + "0" * 64],
        stdout=io.StringIO(),
    )

    assert status == 1


def test_unknown_schema_returns_typed_failure(monkeypatch) -> None:
    def fail(*args: object, **kwargs: object) -> dict:
        raise ValueError("unknown registered schema: schemas/missing.v1.schema.json")

    monkeypatch.setattr(proof_plan_cli, "generate_proof_plan", fail)
    output = io.StringIO()
    status = cli.main(
        ["proof-plan", "--schema", "schemas/missing.v1.schema.json", "--json"],
        stdout=output,
    )

    assert status == 2
    assert json.loads(output.getvalue())["reasonKind"] == (
        "registered-schema-selection-invalid"
    )
