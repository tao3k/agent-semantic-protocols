"""Shared SCM typed-plan operation schema checks."""

from pathlib import Path

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
DIGEST = "1" * 64


def request() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.provider-syntax-query-request",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "ownerPath": "src/lib.rs",
        "sourceContentDigest": DIGEST,
        "queryDigest": DIGEST,
        "source": "pub fn run() {}\n",
        "plan": {
            "patterns": [
                {
                    "index": 0,
                    "captures": ["function.name"],
                    "nodeTypes": ["function_item"],
                    "fields": ["name"],
                }
            ],
            "captures": ["function.name"],
            "nodeTypes": ["function_item"],
            "fields": ["name"],
            "predicates": [],
        },
    }


def test_provider_syntax_query_request_is_grammarless_and_schema_v1() -> None:
    schema_validator_for(SCHEMAS / "provider-syntax-query-request.schema.json").validate(
        request()
    )
    encoded = str(request())
    for forbidden in ["parser.c", "scanner.c", "grammarLibrary", "tree-sitter-rust"]:
        assert forbidden not in encoded


def test_provider_syntax_query_response_returns_native_fact_spans() -> None:
    response = {
        "schemaId": "agent.semantic-protocols.provider-syntax-query-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "ownerPath": "src/lib.rs",
        "sourceContentDigest": DIGEST,
        "queryDigest": DIGEST,
        "parsed": True,
        "captures": [
            {
                "patternIndex": 0,
                "captureName": "function.name",
                "nativeFactRef": "rust:item:src/lib.rs:1:1:run",
                "sourceByteStart": 7,
                "sourceByteEnd": 10,
            }
        ],
    }
    schema_validator_for(SCHEMAS / "provider-syntax-query-response.schema.json").validate(
        response
    )
