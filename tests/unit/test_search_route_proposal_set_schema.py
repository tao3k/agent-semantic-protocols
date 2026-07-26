from __future__ import annotations

import json
import re
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
FIXTURES = SCHEMAS / "fixtures" / "search-route-proposal-set"
RFC = (
    ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.05-cli-first-harness-ux"
    / "10.05.12-search-query-semantic-abi.org"
)


def load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "fixture_name",
    [
        "ready-serial.v1.json",
        "ready-batch.v1.json",
        "ready-parallel.v1.json",
        "blocked-state-unavailable.v1.json",
    ],
)
def test_route_proposal_set_accepts_v1_packets(fixture_name: str) -> None:
    validator = schema_validator_for(
        SCHEMAS / "search-route-proposal-set.v1.schema.json"
    )
    validator.validate(load_json(FIXTURES / fixture_name))


def test_route_proposal_set_rejects_program_authority() -> None:
    validator = schema_validator_for(
        SCHEMAS / "search-route-proposal-set.v1.schema.json"
    )
    with pytest.raises(ValidationError):
        validator.validate(
            load_json(
                FIXTURES
                / "invalid-blocked-with-program-authority.v1.json"
            )
        )


def test_route_proposal_set_rejects_batch_without_capability() -> None:
    validator = schema_validator_for(
        SCHEMAS / "search-route-proposal-set.v1.schema.json"
    )
    with pytest.raises(ValidationError):
        validator.validate(
            load_json(
                FIXTURES / "invalid-batch-without-capability.v1.json"
            )
        )


def test_rfc_choice_panel_uses_proposal_before_program_admission() -> None:
    blocks = [
        json.loads(block)
        for block in re.findall(
            r"#\+begin_src json\n(.*?)\n#\+end_src",
            RFC.read_text(encoding="utf-8"),
            flags=re.DOTALL | re.IGNORECASE,
        )
    ]
    panel = next(
        block
        for block in blocks
        if block.get("schemaId")
        == "agent.semantic-protocols.search-choice-panel"
    )
    schema_validator_for(
        SCHEMAS / "search-interactive-loop.v1.schema.json"
    ).validate(panel)


@pytest.mark.parametrize(
    "schema_name",
    [
        "context-product-state.v1.schema.json",
        "search-route-proposal-set.v1.schema.json",
        "search-interactive-loop.v1.schema.json",
    ],
)
def test_planner_boundary_schemas_are_valid_draft_2020_12(
    schema_name: str,
) -> None:
    Draft202012Validator.check_schema(load_json(SCHEMAS / schema_name))
