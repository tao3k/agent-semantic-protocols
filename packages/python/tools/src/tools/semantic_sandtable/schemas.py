"""JSON-schema validation helpers for sandtable inputs."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .constants import (
    COVERAGE_POLICY_SCHEMA_PATH,
    RECEIPT_SCHEMA_PATH,
    SCENARIO_SCHEMA_PATH,
)
from .models import CoveragePolicyLoadError, ReceiptLoadError, ScenarioLoadError


def validate_scenario_schema(repo_root: Path, path: Path, scenario: Any) -> None:
    schema_path = repo_root / SCENARIO_SCHEMA_PATH
    if not schema_path.exists():
        return
    try:
        from jsonschema import Draft202012Validator
    except ImportError as error:
        raise ScenarioLoadError(
            "scenario schema validation requires jsonschema; run through `uv run semantic-sandtable`"
        ) from error
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise ScenarioLoadError(f"failed to load scenario schema: {error}") from error

    validator = Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(scenario), key=lambda error: list(error.path))
    if errors:
        messages = []
        for error in errors[:3]:
            location = ".".join(str(part) for part in error.path) or "$"
            messages.append(f"{location}: {error.message}")
        if len(errors) > 3:
            messages.append(f"... {len(errors) - 3} more")
        relative = path.relative_to(repo_root) if path.is_relative_to(repo_root) else path
        raise ScenarioLoadError(f"{relative} failed schema validation: {'; '.join(messages)}")


def validate_receipt_schema(repo_root: Path, path: Path, receipt: Any) -> None:
    schema_path = repo_root / RECEIPT_SCHEMA_PATH
    if not schema_path.exists():
        return
    try:
        from jsonschema import Draft202012Validator
    except ImportError as error:
        raise ReceiptLoadError(
            "receipt schema validation requires jsonschema; run through `uv run semantic-sandtable`"
        ) from error
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise ReceiptLoadError(f"failed to load receipt schema: {error}") from error

    validator = Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(receipt), key=lambda error: list(error.path))
    if errors:
        messages = []
        for error in errors[:3]:
            location = ".".join(str(part) for part in error.path) or "$"
            messages.append(f"{location}: {error.message}")
        if len(errors) > 3:
            messages.append(f"... {len(errors) - 3} more")
        relative = path.relative_to(repo_root) if path.is_relative_to(repo_root) else path
        raise ReceiptLoadError(f"{relative} failed receipt schema validation: {'; '.join(messages)}")


def validate_coverage_policy_schema(repo_root: Path, path: Path, policy: Any) -> None:
    schema_path = repo_root / COVERAGE_POLICY_SCHEMA_PATH
    if not schema_path.exists():
        return
    try:
        from jsonschema import Draft202012Validator
    except ImportError as error:
        raise CoveragePolicyLoadError(
            "coverage policy validation requires jsonschema; run through `uv run semantic-sandtable`"
        ) from error
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise CoveragePolicyLoadError(
            f"failed to load coverage policy schema: {error}"
        ) from error

    validator = Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(policy), key=lambda error: list(error.path))
    if errors:
        messages = []
        for error in errors[:3]:
            location = ".".join(str(part) for part in error.path) or "$"
            messages.append(f"{location}: {error.message}")
        if len(errors) > 3:
            messages.append(f"... {len(errors) - 3} more")
        relative = path.relative_to(repo_root) if path.is_relative_to(repo_root) else path
        raise CoveragePolicyLoadError(
            f"{relative} failed schema validation: {'; '.join(messages)}"
        )
