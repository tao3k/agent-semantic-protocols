"""Validate package-local copies of shared semantic schemas."""

from __future__ import annotations

import json
import sys
from pathlib import Path


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "packages/python/src"))

from tools.schema_profiles import (  # noqa: E402
    LANGUAGE_SCHEMA_PROFILES,
    assert_language_schema_profiles,
)


def _load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def test_language_package_profiled_schema_copies_match_protocol_root() -> None:
    for profile in LANGUAGE_SCHEMA_PROFILES:
        for schema_name in profile.shared_schema_files:
            root_schema = _load_json(_ROOT / "schemas" / schema_name)
            package_schema = _load_json(
                _ROOT / profile.package_root / "schemas" / schema_name
            )
            assert package_schema == root_schema, (
                f"{profile.package_root}:{schema_name}"
            )


def test_language_package_schema_directories_match_profiles() -> None:
    assert_language_schema_profiles(_ROOT)
