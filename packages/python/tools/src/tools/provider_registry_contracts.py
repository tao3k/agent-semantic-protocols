"""Validate real provider registry output against shared query contracts."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from tools.paths import repo_root as default_repo_root
from tools.provider_registry_query_contract import (
    query_descriptor_errors,
    query_descriptors,
    single_language,
)
from tools.provider_registry_runtime import (
    load_json,
    provider_registry,
    provider_registry_with_env,
    resolve_asp_bin,
)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    failures = validate_provider_registries(
        args.repo_root.resolve(),
        provider_ids=args.provider,
        asp_bin=args.asp_bin,
    )
    if failures:
        for failure in failures:
            sys.stderr.write(f"{failure}\n")
        return 1
    return 0


def validate_provider_registries(
    repo_root: Path | None = None,
    *,
    provider_ids: list[str] | None = None,
    asp_bin: str | None = None,
    env: dict[str, str] | None = None,
) -> list[str]:
    root = (repo_root or default_repo_root()).resolve()
    asp = resolve_asp_bin(asp_bin)
    validator = _language_registry_validator(root)
    failures: list[str] = []
    for provider in provider_ids or ["rust", "typescript", "python"]:
        failures.extend(_provider_failures(asp, provider, root, validator, env=env))
    return failures


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
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
    args = parser.parse_args(argv)
    if args.provider is None:
        args.provider = ["rust", "typescript", "python"]
    return args


def _language_registry_validator(root: Path) -> Draft202012Validator:
    schema_dir = root / "schemas"
    schema = load_json(schema_dir / "semantic-language-registry.v1.schema.json")
    local_schemas = [load_json(path) for path in sorted(schema_dir.glob("*.schema.json"))]
    registry = Registry().with_resources(
        (schema_id, Resource.from_contents(local_schema))
        for local_schema in local_schemas
        if isinstance((schema_id := local_schema.get("$id")), str)
    )
    return Draft202012Validator(schema, registry=registry)


def _provider_failures(
    asp_bin: str,
    provider: str,
    root: Path,
    validator: Draft202012Validator,
    *,
    env: dict[str, str] | None,
) -> list[str]:
    if env is None:
        registry_result = provider_registry(asp_bin, provider, root)
    else:
        registry_result = provider_registry_with_env(asp_bin, provider, root, env=env)
    if registry_result.error is not None:
        return [registry_result.error]

    assert registry_result.registry is not None
    registry = registry_result.registry
    schema_errors = sorted(validator.iter_errors(registry), key=str)
    if schema_errors:
        return [
            f"{provider}: schema validation failed: "
            f"{_format_schema_error(schema_errors[0])}"
        ]

    descriptor_errors = query_descriptor_errors(provider, registry)
    if descriptor_errors:
        return descriptor_errors

    _write_provider_summary(provider, registry)
    return []


def _format_schema_error(error: Any) -> str:
    path = ".".join(str(part) for part in error.absolute_path)
    if path:
        return f"{path}: {error.message}"
    return error.message


def _write_provider_summary(provider: str, registry: dict[str, Any]) -> None:
    language = single_language(registry)
    descriptors = query_descriptors(language)
    catalog_count = sum(len(desc.get("queryCatalogs", [])) for desc in descriptors)
    sys.stdout.write(
        f"{provider} ok "
        f"queryDescriptors={len(descriptors)} "
        f"catalogs={catalog_count}\n"
    )


if __name__ == "__main__":
    raise SystemExit(main())
