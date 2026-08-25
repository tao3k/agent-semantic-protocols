"""Behavioral contracts for Schema reference and definition graph ownership."""

from pathlib import Path

from asp_schema_manager.catalog import SchemaDocument
from asp_schema_manager.schema_graph import definition_usage, schema_edges


def _document(
    name: str,
    identifier: str,
    value: dict[str, object],
    references: tuple[str, ...] = (),
) -> SchemaDocument:
    return SchemaDocument(
        path=Path(name),
        relative_path=f"schemas/{name}",
        value={"$id": identifier, **value},
        schema_identifier=identifier,
        protocol_schema_id=None,
        references=references,
        byte_length=1,
    )


def test_duplicate_schema_identifiers_are_rejected() -> None:
    documents = [
        _document("one.schema.json", "https://example/schema", {}),
        _document("two.schema.json", "https://example/schema", {}),
    ]

    _edges, _inbound, diagnostics = schema_edges(documents)

    assert [item["code"] for item in diagnostics] == [
        "duplicate-schema-identifier"
    ]


def test_anchor_reference_resolves_to_target_document() -> None:
    target = _document(
        "target.schema.json",
        "https://example/target.schema.json",
        {"$defs": {"shared": {"$anchor": "shared", "type": "string"}}},
    )
    source = _document(
        "source.schema.json",
        "https://example/source.schema.json",
        {"$ref": "target.schema.json#shared"},
        ("target.schema.json#shared",),
    )

    edges, inbound, diagnostics = schema_edges([source, target])

    assert diagnostics == []
    assert edges[source.relative_path] == {target.relative_path}
    assert inbound[target.relative_path] == 1


def test_definition_usage_follows_transitive_definition_references() -> None:
    document = _document(
        "definitions.schema.json",
        "https://example/definitions.schema.json",
        {
            "$ref": "#/$defs/outer",
            "$defs": {
                "outer": {"$ref": "#/$defs/inner"},
                "inner": {"type": "string"},
                "unused": {"type": "boolean"},
            },
        },
        ("#/$defs/outer", "#/$defs/inner"),
    )

    used, unused = definition_usage([document])

    assert used == {
        (document.relative_path, "/$defs/outer"),
        (document.relative_path, "/$defs/inner"),
    }
    assert unused[document.relative_path] == ["/$defs/unused"]
