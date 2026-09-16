# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import copy
import json
from pathlib import Path

import pytest
from blake3 import blake3
from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]


def load_json(path: str) -> dict:
    return json.loads((ROOT / path).read_text())


@pytest.fixture(scope="module")
def validator() -> Draft202012Validator:
    schema = load_json("schemas/semantic-projection.v1.schema.json")
    payload_schema = load_json("schemas/callable-skeleton.schema.json")
    Draft202012Validator.check_schema(schema)
    registry = Registry().with_resources(
        [(schema["$id"], Resource.from_contents(schema)),
         (payload_schema["$id"], Resource.from_contents(payload_schema))]
    )
    return Draft202012Validator(schema, registry=registry)


@pytest.fixture()
def fixture() -> dict:
    return load_json("schemas/fixtures/semantic-projection.callable-skeleton.v1.json")


def test_callable_skeleton_is_payload_of_shared_projection(validator, fixture):
    validator.validate(fixture)
    assert fixture["schemaId"] == "agent.semantic-protocols.semantic-projection"
    assert fixture["schemaVersion"] == "1"
    assert fixture["projectionKind"] == "callable-skeleton"
    assert fixture["payloadSchemaId"] == "agent.semantic-protocols.callable-skeleton"
    canonical = json.dumps(
        fixture["payload"], separators=(",", ":"), sort_keys=True
    ).encode()
    assert fixture["payloadDigest"] == "blake3-256:" + blake3(canonical).hexdigest()


def test_specialized_legacy_envelope_is_rejected(validator, fixture):
    candidate = copy.deepcopy(fixture)
    candidate["schemaId"] = "agent.semantic-protocols.callable-" + "skeleton-projection"
    assert list(validator.iter_errors(candidate))


@pytest.mark.parametrize("field", ["evidenceContextRef", "payloadDigest"])
def test_projection_authority_digest_is_mandatory(validator, fixture, field):
    candidate = copy.deepcopy(fixture)
    del candidate[field]
    assert list(validator.iter_errors(candidate))


def test_legacy_callable_skeleton_schema_is_removed():
    legacy_name = "callable-skeleton-" + "projection.v1.schema.json"
    assert not (ROOT / "schemas" / legacy_name).exists()
    assert not any(
        ROOT.glob("languages/**/schemas/" + legacy_name)
    )
