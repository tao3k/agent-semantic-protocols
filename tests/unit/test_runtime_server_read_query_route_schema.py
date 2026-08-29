import json
from pathlib import Path

from jsonschema import Draft202012Validator, RefResolver


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
SCHEMA_PATH = SCHEMAS / "runtime-server-read-query-route.v1.schema.json"


def validator() -> Draft202012Validator:
    schema = json.loads(SCHEMA_PATH.read_text())
    return Draft202012Validator(
        schema,
        resolver=RefResolver(base_uri=SCHEMA_PATH.as_uri(), referrer=schema),
    )


def valid_request() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.runtime-server-read-query-request.v1",
        "schemaVersion": "1",
        "requestId": "request-1",
        "operation": "exact-query",
        "accessMode": "read-only",
        "transportAuthority": "global-runtime-server",
        "projectId": "repo-1",
        "workspaceIdentity": "workspace-1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "selector": "rust://src/lib.rs#item/function/run",
        "projection": "source",
        "budgetMicros": 800000,
    }


def valid_unavailable_receipt() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.runtime-server-read-query-receipt.v1",
        "schemaVersion": "1",
        "requestId": "request-1",
        "projectId": "repo-1",
        "workspaceIdentity": "workspace-1",
        "route": "global-runtime-internal-workspace",
        "state": "unavailable",
        "source": "none",
        "selector": "rust://src/lib.rs#item/function/run",
        "projection": "source",
        "cachedOwnerDigest": None,
        "liveOwnerDigest": None,
        "cacheFresh": False,
        "generationPublished": False,
        "workspaceOwnerEndpointExposed": False,
        "reasonKind": "provider-native-exact-unavailable",
        "retryAfterMs": 250,
        "nextAction": "asp install language rust --project . --reconcile-receipt",
        "elapsedMicros": 800000,
    }


def test_request_requires_global_read_only_authority() -> None:
    value = valid_request()
    assert not list(validator().iter_errors(value))
    value["transportAuthority"] = "workspace-owner"
    assert list(validator().iter_errors(value))


def test_unavailable_receipt_is_typed_and_actionable() -> None:
    value = valid_unavailable_receipt()
    assert not list(validator().iter_errors(value))
    value["nextAction"] = None
    assert list(validator().iter_errors(value))


def test_public_receipt_cannot_expose_owner_endpoint_or_publish() -> None:
    value = valid_unavailable_receipt()
    value["workspaceOwnerEndpointExposed"] = True
    assert list(validator().iter_errors(value))
    value = valid_unavailable_receipt()
    value["generationPublished"] = True
    assert list(validator().iter_errors(value))


def test_provider_native_fallback_is_not_part_of_the_contract() -> None:
    value = valid_unavailable_receipt()
    value.update(
        state="fallback-projected",
        source="provider-native-exact",
        reasonKind=None,
        retryAfterMs=None,
        nextAction=None,
    )
    assert list(validator().iter_errors(value))
