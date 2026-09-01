"""Cache qualification is explicit, scoped, and content-bound."""

from jsonschema import Draft202012Validator

from tests.unit.live_corpus_search_query_qualification_support import (
    CACHE_STATE_RECEIPT_SCHEMA_PATH,
    CACHE_STATE_REQUEST_SCHEMA_PATH,
    load_json,
)


def test_cache_state_contract_is_scoped_and_content_bound() -> None:
    request_schema = load_json(CACHE_STATE_REQUEST_SCHEMA_PATH)
    receipt_schema = load_json(CACHE_STATE_RECEIPT_SCHEMA_PATH)
    Draft202012Validator.check_schema(request_schema)
    Draft202012Validator.check_schema(receipt_schema)

    required = set(request_schema["required"])
    assert {
        "resourceId",
        "languageId",
        "providerId",
        "artifactDigest",
        "cacheState",
        "prepareAction",
        "mutationScope",
        "expectedGenerationDigest",
        "expectedRootDigest",
    } <= required
    receipt_properties = receipt_schema["properties"]
    assert receipt_properties["sourceWorkspaceMutationCount"]["const"] == 0
    assert receipt_properties["globalCacheMutationCount"]["const"] == 0
    assert receipt_properties["filesystemDeleteCount"]["const"] == 0
