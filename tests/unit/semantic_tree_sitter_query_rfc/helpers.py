"""Shared fixtures for semantic tree-sitter query RFC tests."""

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
RFC_PATH = REPO_ROOT / "rfcs" / "semantic-tree-sitter-query-protocol.org"
SCHEMA_README_PATH = REPO_ROOT / "schemas" / "README.md"


def missing_terms(text: str, required_terms: list[str]) -> list[str]:
    return [term for term in required_terms if term not in text]


def present_terms(text: str, terms: list[str]) -> list[str]:
    return [term for term in terms if term in text]
