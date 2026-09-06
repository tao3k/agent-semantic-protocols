# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Classify JSON Schema keyword occurrences for logical projection."""

from __future__ import annotations

from collections.abc import Iterator
import hashlib


SUPPORTED_KEYWORDS = frozenset(
    {
        "$anchor",
        "$defs",
        "$dynamicAnchor",
        "$dynamicRef",
        "$id",
        "$ref",
        "$schema",
        "additionalProperties",
        "allOf",
        "anyOf",
        "const",
        "contains",
        "dependentRequired",
        "dependentSchemas",
        "else",
        "enum",
        "exclusiveMaximum",
        "exclusiveMinimum",
        "if",
        "items",
        "maxContains",
        "maximum",
        "maxItems",
        "maxLength",
        "maxProperties",
        "minContains",
        "minimum",
        "minItems",
        "minLength",
        "minProperties",
        "multipleOf",
        "not",
        "oneOf",
        "pattern",
        "patternProperties",
        "prefixItems",
        "properties",
        "propertyNames",
        "required",
        "then",
        "type",
        "unevaluatedItems",
        "unevaluatedProperties",
        "uniqueItems",
    }
)

ANNOTATION_KEYWORDS = frozenset(
    {
        "$comment",
        "searchPlaybookContract",
        "contentEncoding",
        "contentMediaType",
        "contentSchema",
        "default",
        "deprecated",
        "description",
        "examples",
        "format",
        "readOnly",
        "title",
        "writeOnly",
    }
)

SCHEMA_MAP_KEYWORDS = frozenset(
    {"$defs", "dependentSchemas", "patternProperties", "properties"}
)


def keyword_facts(value: dict, owner: str) -> list[dict[str, str]]:
    facts = [
        {
            "id": _fact_id(owner, pointer, keyword),
            "owner": owner,
            "field": pointer,
            "role": "executable-selector" if supported else "display-only",
            "keyword": keyword,
            "support": "supported" if supported else "unsupported",
        }
        for pointer, keyword, supported in _keyword_occurrences(value)
    ]
    return sorted(facts, key=lambda item: (item["field"], item["keyword"]))


def _keyword_occurrences(
    value: object, pointer: str = ""
) -> Iterator[tuple[str, str, bool]]:
    if isinstance(value, bool):
        yield pointer or "/", "booleanSchema", True
        return
    if not isinstance(value, dict):
        return
    for keyword, child in sorted(value.items()):
        child_pointer = f"{pointer}/{_escape_pointer(keyword)}"
        supported = keyword in SUPPORTED_KEYWORDS
        if supported or keyword in ANNOTATION_KEYWORDS or keyword.startswith("$"):
            yield child_pointer, keyword, supported
        else:
            yield child_pointer, keyword, False
        yield from _child_schema_keywords(keyword, child, child_pointer)


def _child_schema_keywords(
    keyword: str, child: object, pointer: str
) -> Iterator[tuple[str, str, bool]]:
    if keyword in SCHEMA_MAP_KEYWORDS and isinstance(child, dict):
        for name, schema in sorted(child.items()):
            yield from _keyword_occurrences(
                schema, f"{pointer}/{_escape_pointer(name)}"
            )
        return
    if keyword in {"allOf", "anyOf", "oneOf", "prefixItems"} and isinstance(
        child, list
    ):
        for index, schema in enumerate(child):
            yield from _keyword_occurrences(schema, f"{pointer}/{index}")
        return
    if keyword in {
        "additionalProperties",
        "contains",
        "contentSchema",
        "else",
        "if",
        "items",
        "not",
        "propertyNames",
        "then",
        "unevaluatedItems",
        "unevaluatedProperties",
    }:
        yield from _keyword_occurrences(child, pointer)


def _fact_id(owner: str, pointer: str, keyword: str) -> str:
    digest = hashlib.sha256(f"{owner}\0{pointer}\0{keyword}".encode()).hexdigest()[:20]
    return f"schema.fact.{digest}"


def _escape_pointer(token: str) -> str:
    return token.replace("~", "~0").replace("/", "~1")
