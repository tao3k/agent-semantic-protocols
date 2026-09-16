# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shared fixtures for semantic tree-sitter query RFC tests."""

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
RFC_PATH = (
    REPO_ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.11-semantic-tree-sitter-query-protocol.org"
)
SCHEMA_README_PATH = REPO_ROOT / "schemas" / "README.md"


def missing_terms(text: str, required_terms: list[str]) -> list[str]:
    return [term for term in required_terms if term not in text]


def present_terms(text: str, terms: list[str]) -> list[str]:
    return [term for term in terms if term in text]
