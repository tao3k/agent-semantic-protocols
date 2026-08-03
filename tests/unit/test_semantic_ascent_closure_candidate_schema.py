import json
from pathlib import Path

from jsonschema import Draft202012Validator


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = (
    REPOSITORY_ROOT / "schemas" / "semantic-ascent-closure-candidate.v1.schema.json"
)
FIXTURE_ROOT = (
    REPOSITORY_ROOT / "schemas" / "fixtures" / "semantic-ascent-closure-candidate"
)


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def validator() -> Draft202012Validator:
    return Draft202012Validator(load_json(SCHEMA_PATH))


def test_grounded_closure_candidate_is_schema_valid() -> None:
    errors = list(
        validator().iter_errors(
            load_json(FIXTURE_ROOT / "valid-grounded-fixed-point-candidate.v1.json")
        )
    )
    assert errors == []


def test_closure_candidate_cannot_claim_proved_authority() -> None:
    errors = list(
        validator().iter_errors(
            load_json(FIXTURE_ROOT / "invalid-proved-authority.v1.json")
        )
    )
    assert errors


def test_derived_fact_requires_a_derivation_trace() -> None:
    errors = list(
        validator().iter_errors(
            load_json(FIXTURE_ROOT / "invalid-derived-fact-without-trace.v1.json")
        )
    )
    assert errors
