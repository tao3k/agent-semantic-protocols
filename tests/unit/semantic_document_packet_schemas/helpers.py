"""Shared helpers for document packet schema tests."""

from __future__ import annotations

import importlib.util
from pathlib import Path
from typing import Callable


REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_VALIDATION_PATH = REPO_ROOT / "tests" / "unit" / "schema_validation.py"
SCHEMA_VALIDATION_SPEC = importlib.util.spec_from_file_location(
    "schema_validation", SCHEMA_VALIDATION_PATH
)
assert SCHEMA_VALIDATION_SPEC is not None
assert SCHEMA_VALIDATION_SPEC.loader is not None
SCHEMA_VALIDATION_MODULE = importlib.util.module_from_spec(SCHEMA_VALIDATION_SPEC)
SCHEMA_VALIDATION_SPEC.loader.exec_module(SCHEMA_VALIDATION_MODULE)

schema_validator_for: Callable[[Path], object] = (
    SCHEMA_VALIDATION_MODULE.schema_validator_for
)
