import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"


def validator() -> Draft202012Validator:
    host_schema = Resource.from_contents(
        json.loads((SCHEMAS / "semantic-hook-host-acceptance.v1.schema.json").read_text())
    )
    registry = Registry().with_resource(
        "https://tao3k.github.io/agent-semantic-protocols/schemas/semantic-hook-host-acceptance.v1.schema.json",
        host_schema,
    )
    schema = json.loads((SCHEMAS / "hook-enablement-acceptance.v2.schema.json").read_text())
    return Draft202012Validator(schema, registry=registry)


def fixture(name: str) -> dict:
    return json.loads(
        (SCHEMAS / "fixtures" / "hook-enablement-acceptance" / name).read_text()
    )


def test_probe_ready_and_final_ready_receipts_are_valid() -> None:
    gate = validator()
    gate.validate(fixture("probe-ready.v2.json"))
    gate.validate(fixture("ready.v2.json"))


def test_probe_ready_cannot_claim_host_acceptance() -> None:
    instance = fixture("ready.v2.json")
    instance["status"] = "probe-ready"
    instance["reasonKind"] = "host-delivery-evidence-required"
    assert list(validator().iter_errors(instance))


def test_final_ready_requires_accepted_generation_bound_host_evidence() -> None:
    instance = fixture("ready.v2.json")
    instance["hostAcceptance"]["generationBoundDenyObserved"] = False
    assert list(validator().iter_errors(instance))
# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
