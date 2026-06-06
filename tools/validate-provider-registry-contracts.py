#!/usr/bin/env python3
"""Validate real provider registry output against shared query contracts."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


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


def main() -> int:
    args = _parse_args()
    repo_root = args.repo_root.resolve()
    asp_bin = _resolve_asp_bin(args.asp_bin)
    schema = _load_json(repo_root / "schemas" / "semantic-language-registry.v1.schema.json")
    validator = Draft202012Validator(schema)

    failures: list[str] = []
    for provider in args.provider:
        registry_result = _provider_registry(asp_bin, provider, repo_root)
        if registry_result.error is not None:
            failures.append(registry_result.error)
            continue

        assert registry_result.registry is not None
        registry = registry_result.registry
        schema_errors = sorted(validator.iter_errors(registry), key=str)
        if schema_errors:
            failures.append(
                f"{provider}: schema validation failed: {_format_schema_error(schema_errors[0])}"
            )
            continue

        descriptor_errors = _query_descriptor_errors(provider, registry)
        if descriptor_errors:
            failures.extend(descriptor_errors)
            continue

        language = _single_language(registry)
        query_descriptors = _query_descriptors(language)
        catalog_count = sum(len(desc.get("queryCatalogs", [])) for desc in query_descriptors)
        sys.stdout.write(
            f"{provider} ok "
            f"queryDescriptors={len(query_descriptors)} "
            f"catalogs={catalog_count}\n"
        )

    if failures:
        for failure in failures:
            sys.stderr.write(f"{failure}\n")
        return 1
    return 0


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run real provider registry commands and validate tree-sitter query "
            "descriptor contracts."
        )
    )
    parser.add_argument(
        "repo_root",
        nargs="?",
        default=".",
        type=Path,
        help="ASP repository root passed to provider doctor commands.",
    )
    parser.add_argument(
        "--asp-bin",
        default=None,
        help="asp executable to run. Defaults to SEMANTIC_AGENT_PROTOCOL_BIN or PATH.",
    )
    parser.add_argument(
        "--provider",
        action="append",
        choices=("rust", "typescript", "python"),
        help="Provider language to validate. Repeatable. Defaults to rust/typescript/python.",
    )
    args = parser.parse_args()
    if args.provider is None:
        args.provider = ["rust", "typescript", "python"]
    return args


def _resolve_asp_bin(configured: str | None) -> str:
    candidate = configured or _env_asp_bin() or "asp"
    if "/" in candidate:
        return candidate
    resolved = shutil.which(candidate)
    if resolved is None:
        raise SystemExit(f"asp binary not found: {candidate}")
    return resolved


def _env_asp_bin() -> str | None:
    from os import environ

    return environ.get("SEMANTIC_AGENT_PROTOCOL_BIN")


def _load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        raise SystemExit(f"{path}: expected JSON object")
    return data


@dataclass(frozen=True, slots=True)
class RegistryResult:
    registry: dict[str, Any] | None = None
    error: str | None = None


def _provider_registry(asp_bin: str, provider: str, repo_root: Path) -> RegistryResult:
    argv = [asp_bin, provider, "agent", "doctor", "--json", str(repo_root)]
    try:
        completed = subprocess.run(
            argv,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        return RegistryResult(error=f"{provider}: doctor command timed out after 30s")

    if completed.returncode != 0:
        stderr = completed.stderr.strip().splitlines()
        detail = stderr[-1] if stderr else f"exit={completed.returncode}"
        return RegistryResult(error=f"{provider}: doctor command failed: {detail}")

    try:
        registry = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        return RegistryResult(error=f"{provider}: invalid JSON: {error}")

    if not isinstance(registry, dict):
        return RegistryResult(error=f"{provider}: registry JSON must be an object")
    return RegistryResult(registry=registry)


def _format_schema_error(error: Any) -> str:
    path = ".".join(str(part) for part in error.absolute_path)
    if path:
        return f"{path}: {error.message}"
    return error.message


def _single_language(registry: dict[str, Any]) -> dict[str, Any]:
    languages = registry.get("languages")
    if not isinstance(languages, list) or len(languages) != 1:
        raise ValueError("registry must contain exactly one language entry")
    language = languages[0]
    if not isinstance(language, dict):
        raise ValueError("language entry must be an object")
    return language


def _query_descriptors(language: dict[str, Any]) -> list[dict[str, Any]]:
    descriptors = language.get("methodDescriptors")
    if not isinstance(descriptors, list):
        return []
    result: list[dict[str, Any]] = []
    for descriptor in descriptors:
        if not isinstance(descriptor, dict):
            continue
        packet_schemas = descriptor.get("packetSchemas")
        if not isinstance(packet_schemas, list):
            continue
        if TREE_SITTER_QUERY_PACKET not in packet_schemas:
            continue
        if descriptor.get("command") != "query":
            continue
        result.append(descriptor)
    return result


def _query_descriptor_errors(provider: str, registry: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    try:
        language = _single_language(registry)
    except ValueError as error:
        return [f"{provider}: {error}"]

    descriptors = _query_descriptors(language)
    if not descriptors:
        return [f"{provider}: no query descriptor advertises {TREE_SITTER_QUERY_PACKET}"]

    for descriptor in descriptors:
        method = descriptor.get("method", "<unknown>")
        for field in REQUIRED_QUERY_DESCRIPTOR_FIELDS:
            if field not in descriptor:
                errors.append(f"{provider}:{method}: missing {field}")
        errors.extend(_value_errors(provider, method, descriptor))
    return errors


def _value_errors(provider: str, method: Any, descriptor: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for field in ("adapterModes", "sourceAuthorities", "executionBackends", "renderProfiles"):
        value = descriptor.get(field)
        if not isinstance(value, list) or not value:
            errors.append(f"{provider}:{method}: {field} must be a non-empty array")
    if "native-parser" not in descriptor.get("executionBackends", []):
        errors.append(f"{provider}:{method}: executionBackends must include native-parser")
    if descriptor.get("unsupportedPatternBehavior") not in {"diagnostic", "reject", "ignore"}:
        errors.append(f"{provider}:{method}: unsupportedPatternBehavior is invalid")
    code_output = descriptor.get("codeOutput")
    if not isinstance(code_output, dict) or code_output.get("mode") != "pure-code":
        errors.append(f"{provider}:{method}: codeOutput.mode must be pure-code")
    if descriptor.get("method") == "query":
        catalogs = descriptor.get("queryCatalogs")
        if not isinstance(catalogs, list) or not catalogs:
            errors.append(f"{provider}:{method}: canonical query method must declare catalogs")
        else:
            for catalog in catalogs:
                if not isinstance(catalog, dict):
                    errors.append(f"{provider}:{method}: queryCatalog entry must be an object")
                    continue
                if catalog.get("sourceDelivery") != "provider-binary-embedded":
                    catalog_id = catalog.get("id", "<unknown>")
                    errors.append(
                        f"{provider}:{method}:{catalog_id}: "
                        "sourceDelivery must be provider-binary-embedded"
                    )
    return errors


if __name__ == "__main__":
    raise SystemExit(main())
