from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA = _REPO_ROOT / "schemas/semantic-graph-turbo-resident-status.v1.schema.json"
_FIXTURES = _REPO_ROOT / "schemas/fixtures/semantic-graph-turbo-resident"


def _load(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def test_graph_turbo_resident_status_fixtures() -> None:
    validator = Draft202012Validator(_load(_SCHEMA))
    for name in (
        "valid-daemon-owned-status.v1.json",
        "valid-unavailable-status.v1.json",
    ):
        validator.validate(_load(_FIXTURES / name))

    invalid = _load(_FIXTURES / "invalid-healthy-status-without-process.v1.json")
    assert list(validator.iter_errors(invalid))
