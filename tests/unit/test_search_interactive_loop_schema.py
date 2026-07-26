import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "search-interactive-loop.v1.schema.json"
CAPABILITY_SCHEMA_PATH = ROOT / "schemas" / "search-loop-capability.v1.schema.json"
FIXTURE_ROOT = ROOT / "schemas" / "fixtures" / "search-interactive-loop"
DEPENDENCY_PATHS = (
    ROOT / "schemas" / "context-product-state.v1.schema.json",
    ROOT / "schemas" / "semantic-command.v1.schema.json",
    ROOT / "schemas" / "canonical-item-selector.v1.schema.json",
)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def validator() -> Draft202012Validator:
    documents = [load_json(SCHEMA_PATH), load_json(CAPABILITY_SCHEMA_PATH)]
    documents.extend(load_json(path) for path in DEPENDENCY_PATHS)
    registry = Registry().with_resources(
        (document["$id"], Resource.from_contents(document))
        for document in documents
    )
    return Draft202012Validator(documents[0], registry=registry)


def capability_validator() -> Draft202012Validator:
    documents = [load_json(CAPABILITY_SCHEMA_PATH), load_json(SCHEMA_PATH)]
    documents.extend(load_json(path) for path in DEPENDENCY_PATHS)
    registry = Registry().with_resources(
        (document["$id"], Resource.from_contents(document))
        for document in documents
    )
    return Draft202012Validator(documents[0], registry=registry)


@pytest.mark.parametrize(
    "fixture_name",
    [
        "running-parallel.v1.json",
        "query-handoff.v1.json",
    ],
)
def test_search_interactive_loop_accepts_issued_actions(fixture_name: str) -> None:
    validator().validate(load_json(FIXTURE_ROOT / fixture_name))


@pytest.mark.parametrize(
    "fixture_name",
    [
        "invalid-running-without-poll-action.v1.json",
        "invalid-joined-with-poll-action.v1.json",
    ],
)
def test_search_interactive_loop_rejects_missing_or_stale_poll_actions(
    fixture_name: str,
) -> None:
    with pytest.raises(ValidationError):
        validator().validate(load_json(FIXTURE_ROOT / fixture_name))


def test_search_interactive_loop_schema_is_valid_draft_2020_12() -> None:
    Draft202012Validator.check_schema(load_json(SCHEMA_PATH))


@pytest.mark.parametrize(
    "fixture_name",
    [
        "advance-capability.v1.json",
        "consumed-advance-capability.v1.json",
        "poll-capability.v1.json",
    ],
)
def test_search_loop_capability_accepts_typed_ledger_records(
    fixture_name: str,
) -> None:
    capability_validator().validate(load_json(FIXTURE_ROOT / fixture_name))


@pytest.mark.parametrize(
    "fixture_name",
    [
        "invalid-poll-capability-consumed.v1.json",
        "invalid-capability-with-raw-token.v1.json",
    ],
)
def test_search_loop_capability_rejects_invalid_persistence(
    fixture_name: str,
) -> None:
    with pytest.raises(ValidationError):
        capability_validator().validate(load_json(FIXTURE_ROOT / fixture_name))


def test_search_loop_capability_schema_is_valid_draft_2020_12() -> None:
    Draft202012Validator.check_schema(load_json(CAPABILITY_SCHEMA_PATH))
