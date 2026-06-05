"""Validate semantic-tree-sitter grammar profile schema fixtures."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = _REPO_ROOT / "schemas/semantic-tree-sitter-grammar-profile.v1.schema.json"
_RUST_PROFILE_PATH = (
    _REPO_ROOT
    / "languages/rust-lang-project-harness/tree-sitter/tree-sitter-rust/grammar-profile.json"
)
_PYTHON_PROFILE_PATH = (
    _REPO_ROOT
    / "languages/python-lang-project-harness/tree-sitter/tree-sitter-python/grammar-profile.json"
)


def _validator() -> Draft202012Validator:
    with _SCHEMA_PATH.open() as handle:
        return Draft202012Validator(json.load(handle))


def _rust_profile() -> dict[str, Any]:
    return _profile(_RUST_PROFILE_PATH)


def _python_profile() -> dict[str, Any]:
    return _profile(_PYTHON_PROFILE_PATH)


def _profile(path: Path) -> dict[str, Any]:
    with path.open() as handle:
        return json.load(handle)


def _errors(profile: dict[str, Any]) -> list[str]:
    return [error.message for error in _validator().iter_errors(profile)]


def test_rust_tree_sitter_grammar_profile_is_valid() -> None:
    assert _errors(_rust_profile()) == []


def test_python_tree_sitter_grammar_profile_is_valid() -> None:
    assert _errors(_python_profile()) == []


def test_profile_requires_main_asp_revision_provenance() -> None:
    profile = _rust_profile()
    del profile["aspWorkspace"]["revision"]

    errors = _errors(profile)

    assert any("'revision' is a required property" in error for error in errors)


def test_profile_rejects_unscoped_capture_names() -> None:
    profile = _rust_profile()
    profile["catalogs"][0]["captures"].append("function")

    errors = _errors(profile)

    assert any("does not match" in error for error in errors)


def test_profile_rejects_unscoped_native_fact_projection_captures() -> None:
    profile = _rust_profile()
    profile["nativeFactProjection"][0]["captures"].append("visibility")

    errors = _errors(profile)

    assert any("does not match" in error for error in errors)


def test_native_fact_projection_captures_are_catalog_declared() -> None:
    profile = _rust_profile()
    captures_by_catalog = {
        catalog["id"]: set(catalog["captures"]) for catalog in profile["catalogs"]
    }

    for projection in profile["nativeFactProjection"]:
        catalog_captures = captures_by_catalog[projection["catalogId"]]
        assert set(projection["captures"]).issubset(catalog_captures)


def test_profile_rejects_unknown_fields() -> None:
    profile = deepcopy(_rust_profile())
    profile["runtimeLoadsProviderPackageSource"] = True

    errors = _errors(profile)

    assert any("Additional properties are not allowed" in error for error in errors)
