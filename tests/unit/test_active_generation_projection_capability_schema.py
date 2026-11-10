# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "active-generation-projection-capability.v1.schema.json"
READY_FIXTURE_PATH = (
    ROOT
    / "schemas"
    / "fixtures"
    / "active-generation-projection-capability.ready.v1.json"
)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def test_ready_projection_capability_accepts_an_explicit_empty_selector_set() -> None:
    schema = load_json(SCHEMA_PATH)
    ready = load_json(READY_FIXTURE_PATH)
    ready_empty = copy.deepcopy(ready)
    ready_empty["selectors"] = []

    Draft202012Validator(schema).validate(ready_empty)


def test_ready_projection_capability_remains_v1() -> None:
    schema = load_json(SCHEMA_PATH)
    ready = load_json(READY_FIXTURE_PATH)

    assert schema["$id"].endswith("active-generation-projection-capability.v1.schema.json")
    assert ready["schemaVersion"] == "1"


def test_provider_catalog_digest_preserves_the_schema_owned_v1_wire_formats() -> None:
    schema = load_json(SCHEMA_PATH)
    ready = load_json(READY_FIXTURE_PATH)
    candidates = {
        "sha256": "sha256:" + "1" * 64,
        "blake3-256": "blake3-256:" + "1" * 64,
        "raw-hex": "1" * 64,
    }
    accepted: list[str] = []

    for name, digest in candidates.items():
        candidate = copy.deepcopy(ready)
        candidate["providerCatalogDigest"] = digest
        if not list(Draft202012Validator(schema).iter_errors(candidate)):
            accepted.append(name)

    assert accepted == ["sha256", "blake3-256", "raw-hex"], (
        f"unexpected v1 digest wire formats: {accepted}"
    )
