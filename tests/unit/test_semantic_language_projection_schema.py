"""Contract tests for query-free language projection artifacts."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "semantic-language-projection.v1.schema.json"
SOURCE_LOCATION_SCHEMA_PATH = ROOT / "schemas" / "semantic-source-location.v1.schema.json"


def projection_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-language-projection",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.language-projection",
        "protocolVersion": "1",
        "languageId": "gerbil-scheme",
        "harness": {
            "harnessId": "gerbil-scheme-language-project-harness",
            "parserAbi": "gerbil-parser-v1",
            "selectorDialect": "gerbil-scheme",
        },
        "sources": [
            {
                "sourceId": "source:src/main.ss",
                "path": "src/main.ss",
                "sourceKind": "source",
            }
        ],
        "owners": [
            {
                "ownerId": "owner:src/main.ss",
                "sourceId": "source:src/main.ss",
                "kind": "module",
                "name": "main",
            }
        ],
        "items": [
            {
                "itemId": "item:src/main.ss:run",
                "ownerId": "owner:src/main.ss",
                "kind": "function",
                "name": "run",
                "selector": "gerbil-scheme://src/main.ss#item/function/run",
            }
        ],
        "relations": [
            {
                "from": {"kind": "source", "id": "source:src/main.ss"},
                "kind": "contains",
                "to": {"kind": "owner", "id": "owner:src/main.ss"},
            },
            {
                "from": {"kind": "owner", "id": "owner:src/main.ss"},
                "kind": "contains",
                "to": {"kind": "item", "id": "item:src/main.ss:run"},
            },
        ],
    }


def validation_errors(packet: dict[str, object]) -> list[str]:
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    source_location_schema = json.loads(
        SOURCE_LOCATION_SCHEMA_PATH.read_text(encoding="utf-8")
    )
    registry = Registry().with_resource(
        source_location_schema["$id"],
        Resource.from_contents(source_location_schema),
    )
    validator = Draft202012Validator(schema, registry=registry)
    return [error.message for error in validator.iter_errors(packet)]


def test_language_projection_accepts_parser_owned_facts() -> None:
    assert validation_errors(projection_packet()) == []


def test_language_projection_rejects_top_level_search_query() -> None:
    packet = deepcopy(projection_packet())
    packet["query"] = "run cache"

    assert validation_errors(packet)


def test_language_projection_rejects_lifecycle_metadata() -> None:
    packet = deepcopy(projection_packet())
    packet["artifact"] = {"artifactId": "Rust-only"}

    assert validation_errors(packet)


def test_language_projection_rejects_rank_inside_item() -> None:
    packet = deepcopy(projection_packet())
    packet["items"][0]["rank"] = 1  # type: ignore[index]

    assert validation_errors(packet)
