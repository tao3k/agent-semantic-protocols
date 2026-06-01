"""Shared constants for semantic sandtable execution."""

from __future__ import annotations

import re
from pathlib import Path


DEFAULT_SCENARIO_GLOB = "sandtables/**/*.json"
SCENARIO_SCHEMA_PATH = Path("schemas/semantic-sandtable-scenario.v1.schema.json")
COVERAGE_POLICY_PATH = Path("sandtables/coverage-policy.json")
COVERAGE_POLICY_SCHEMA_PATH = Path(
    "schemas/semantic-sandtable-coverage-policy.v1.schema.json"
)
RECEIPT_SCHEMA_PATH = Path("schemas/semantic-sandtable-receipt.v1.schema.json")
LARGE_LIBRARY_INTENT_KINDS = (
    "feature-implementation",
    "api-usage",
    "implementation-principle",
)
LARGE_LIBRARY_MIN_TARGETS_PER_LANGUAGE = 3
TOKEN_PATTERN = re.compile(r"\{([A-Za-z_][A-Za-z0-9_]*)\}")
PROJECT_PATH_PATTERN = re.compile(
    r"^(?:\.|(?!/)(?![A-Za-z]:)(?![0-9]+:)(?!\.{1,2}(?:/|$))"
    r"(?!.*(?:^|/)\.{1,2}(?:/|$))(?!.*//)[^\s:\\]+(?:/[^\s:\\]+)*)$"
)
RANK_PREFIXED_PATH_PATTERN = re.compile(
    r"^[0-9]+:[^\s,;)]+$"
)
