"""Tree-sitter query descriptor assertions for provider registries."""

from __future__ import annotations

from typing import Any


TREE_SITTER_QUERY_PACKET = "semantic-tree-sitter-query.v1"

REQUIRED_QUERY_DESCRIPTOR_FIELDS = (
    "adapterModes",
    "sourceAuthorities",
    "executionBackends",
    "renderProfiles",
    "unsupportedPatternBehavior",
    "codeOutput",
    "queryInputForms",
    "grammarId",
    "grammarProfileVersion",
    "grammarProfileSchema",
    "grammarProfilePath",
    "cacheReplay",
)


def query_descriptor_errors(provider: str, registry: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    try:
        language = single_language(registry)
    except ValueError as error:
        return [f"{provider}: {error}"]

    descriptors = query_descriptors(language)
    if not descriptors:
        return [f"{provider}: no query descriptor advertises {TREE_SITTER_QUERY_PACKET}"]

    for descriptor in descriptors:
        method = descriptor.get("method", "<unknown>")
        errors.extend(_missing_field_errors(provider, method, descriptor))
        errors.extend(_value_errors(provider, method, descriptor))
    return errors


def query_descriptors(language: dict[str, Any]) -> list[dict[str, Any]]:
    descriptors = language.get("methodDescriptors")
    if not isinstance(descriptors, list):
        return []
    return [
        descriptor
        for descriptor in descriptors
        if _is_tree_sitter_query_descriptor(descriptor)
    ]


def single_language(registry: dict[str, Any]) -> dict[str, Any]:
    languages = registry.get("languages")
    if not isinstance(languages, list) or len(languages) != 1:
        raise ValueError("registry must contain exactly one language entry")
    language = languages[0]
    if not isinstance(language, dict):
        raise ValueError("language entry must be an object")
    return language


def _is_tree_sitter_query_descriptor(value: object) -> bool:
    if not isinstance(value, dict):
        return False
    packet_schemas = value.get("packetSchemas")
    return (
        isinstance(packet_schemas, list)
        and TREE_SITTER_QUERY_PACKET in packet_schemas
        and value.get("command") == "query"
    )


def _missing_field_errors(
    provider: str, method: Any, descriptor: dict[str, Any]
) -> list[str]:
    return [
        f"{provider}:{method}: missing {field}"
        for field in REQUIRED_QUERY_DESCRIPTOR_FIELDS
        if field not in descriptor
    ]


def _value_errors(provider: str, method: Any, descriptor: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    errors.extend(_non_empty_array_errors(provider, method, descriptor))
    errors.extend(_query_backend_errors(provider, method, descriptor))
    errors.extend(_query_catalog_errors(provider, method, descriptor))
    return errors


def _non_empty_array_errors(
    provider: str, method: Any, descriptor: dict[str, Any]
) -> list[str]:
    fields = ("adapterModes", "sourceAuthorities", "executionBackends", "renderProfiles")
    return [
        f"{provider}:{method}: {field} must be a non-empty array"
        for field in fields
        if not isinstance(descriptor.get(field), list) or not descriptor.get(field)
    ]


def _query_backend_errors(
    provider: str, method: Any, descriptor: dict[str, Any]
) -> list[str]:
    errors: list[str] = []
    if "native-parser" not in descriptor.get("executionBackends", []):
        errors.append(f"{provider}:{method}: executionBackends must include native-parser")
    if descriptor.get("unsupportedPatternBehavior") not in {"diagnostic", "reject", "ignore"}:
        errors.append(f"{provider}:{method}: unsupportedPatternBehavior is invalid")
    code_output = descriptor.get("codeOutput")
    if not isinstance(code_output, dict) or code_output.get("mode") != "pure-code":
        errors.append(f"{provider}:{method}: codeOutput.mode must be pure-code")
    return errors


def _query_catalog_errors(
    provider: str, method: Any, descriptor: dict[str, Any]
) -> list[str]:
    if descriptor.get("method") != "query":
        return []
    catalogs = descriptor.get("queryCatalogs")
    if not isinstance(catalogs, list) or not catalogs:
        return [f"{provider}:{method}: canonical query method must declare catalogs"]
    return [
        error
        for catalog in catalogs
        for error in _catalog_entry_errors(provider, method, catalog)
    ]


def _catalog_entry_errors(provider: str, method: Any, catalog: object) -> list[str]:
    if not isinstance(catalog, dict):
        return [f"{provider}:{method}: queryCatalog entry must be an object"]
    if catalog.get("sourceDelivery") != "provider-binary-embedded":
        catalog_id = catalog.get("id", "<unknown>")
        return [
            f"{provider}:{method}:{catalog_id}: "
            "sourceDelivery must be provider-binary-embedded"
        ]
    return []
