"""Evidence, review, and proof method registry schema tests."""

from .support import (
    language_descriptor_errors,
    language_registry_errors,
    registry_with_descriptor,
)


def test_evidence_review_and_proof_methods_validate() -> None:
    for method, command, schema_id in [
        (
            "proof/pilot",
            "proof",
            "agent.semantic-protocols.semantic-formal-proof-pilot",
        ),
        (
            "review/packet",
            "review",
            "agent.semantic-protocols.semantic-review-packet",
        ),
        (
            "evidence/assurance",
            "evidence",
            "agent.semantic-protocols.semantic-assurance-case",
        ),
    ]:
        registry = registry_with_descriptor(
            {
                "method": method,
                "command": command,
                "input": method.split("/", maxsplit=1)[1],
                "outputSchemaIds": [schema_id],
                "supportsJson": True,
                "supportsCompact": True,
            },
            schemas=[
                {
                    "schemaId": schema_id,
                    "schemaVersion": "1",
                    "path": (
                        f"schemas/{schema_id.rsplit('.', maxsplit=1)[1]}.v1.schema.json"
                    ),
                }
            ],
        )

        assert language_registry_errors(registry) == []


def test_evidence_json_method_requires_output_schema_ids() -> None:
    errors = language_descriptor_errors(
        {
            "method": "evidence/assurance",
            "command": "evidence",
            "input": "assurance",
            "supportsJson": True,
            "supportsCompact": True,
        }
    )

    assert "'outputSchemaIds' is a required property" in errors
