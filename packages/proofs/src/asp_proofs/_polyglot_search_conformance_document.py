# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Search-document framing, control parsing, and validation orchestration."""

from __future__ import annotations

import unicodedata
from collections.abc import Iterable
from typing import Any

from asp_proofs._polyglot_search_conformance_model import (
    ConformanceReceipt,
    Violation,
    receipt,
)
from asp_proofs._polyglot_search_conformance_profiles import parse_gql, parse_logic

SECTION_NAMES = ("SEARCH", "GQL", "LOGIC", "TURBO", "EMIT")
EMIT_KINDS = {"CLOSURE", "EVIDENCE", "FRONTIER"}


def split_sections(document: str) -> tuple[dict[str, str], list[Violation]]:
    sections: dict[str, list[str]] = {}
    current: str | None = None
    violations: list[Violation] = []
    for line_number, line in enumerate(document.splitlines(), start=1):
        stripped = line.strip()
        if not stripped:
            continue
        if line == stripped and stripped in SECTION_NAMES:
            if stripped in sections:
                violations.append(
                    Violation(
                        "duplicate-section",
                        f"/line/{line_number}",
                        f"Section {stripped} occurs more than once.",
                    )
                )
            current = stripped
            sections.setdefault(stripped, [])
            continue
        if current is None:
            violations.append(
                Violation(
                    "content-before-section",
                    f"/line/{line_number}",
                    "Content occurs before a recognized section frame.",
                )
            )
            continue
        sections[current].append(line)
    if "SEARCH" not in sections:
        violations.append(
            Violation("missing-search-section", "/SEARCH", "SEARCH is required.")
        )
    if "EMIT" not in sections:
        violations.append(
            Violation("missing-emit-section", "/EMIT", "EMIT is required.")
        )
    return {
        name: "\n".join(lines).strip() for name, lines in sections.items()
    }, violations


def parse_control(source: str) -> tuple[dict[str, Any], list[Violation]]:
    summary: dict[str, Any] = {"profile": "asp-search-control:1"}
    violations: list[Violation] = []
    for line_number, line in enumerate(source.splitlines(), start=1):
        words = line.strip().split()
        path = f"/SEARCH/{line_number}"
        parsed = _control_field(words)
        if parsed is None:
            violations.append(
                Violation(
                    "unknown-control-clause",
                    path,
                    "Control clause is not in profile v1.",
                )
            )
            continue
        key, value = parsed
        if key in summary:
            violations.append(
                Violation(
                    "duplicate-control-clause",
                    path,
                    f"Control field {key} is duplicated.",
                )
            )
            continue
        if key in {"exposureBudget", "selectionBudget"}:
            try:
                value = int(value)
            except ValueError:
                violations.append(
                    Violation("invalid-budget", path, "Budget must be an integer.")
                )
                continue
        summary[key] = value
    _validate_control_summary(summary, violations)
    return summary, violations


def _control_field(words: list[str]) -> tuple[str, str] | None:
    if len(words) == 2 and words[0] == "SNAPSHOT":
        return "snapshotRef", words[1]
    if len(words) == 3 and words[:2] == ["BUDGET", "NODES"]:
        return "exposureBudget", words[2]
    if len(words) == 4 and words[:3] == ["SELECT", "AT", "MOST"]:
        return "selectionBudget", words[3]
    if len(words) == 2 and words[0] == "CONTINUATION":
        return "continuationRef", words[1]
    return None


def _validate_control_summary(
    summary: dict[str, Any], violations: list[Violation]
) -> None:
    if "snapshotRef" not in summary:
        violations.append(
            Violation("missing-snapshot", "/SEARCH", "SNAPSHOT is required.")
        )
    exposure = summary.get("exposureBudget")
    selection = summary.get("selectionBudget")
    if not isinstance(exposure, int) or not 1 <= exposure <= 10:
        violations.append(
            Violation(
                "invalid-exposure-budget", "/SEARCH/BUDGET", "Exposure must be 1..10."
            )
        )
    if not isinstance(selection, int) or not 1 <= selection <= 3:
        violations.append(
            Violation(
                "invalid-selection-budget", "/SEARCH/SELECT", "Selection must be 1..3."
            )
        )


def validate_search_document(
    document: str, *, registered_predicates: Iterable[str], state_digest: str
) -> ConformanceReceipt:
    violations: list[Violation] = []
    if unicodedata.normalize("NFC", document) != document:
        violations.append(
            Violation(
                "noncanonical-unicode",
                "/",
                "Search document must use NFC normalization.",
            )
        )
    sections, found = split_sections(document)
    violations.extend(found)
    summary: dict[str, Any] = {}
    parsers = {
        "SEARCH": parse_control,
        "GQL": parse_gql,
        "LOGIC": lambda value: parse_logic(value, frozenset(registered_predicates)),
    }
    for section, key in (("SEARCH", "control"), ("GQL", "gql"), ("LOGIC", "logic")):
        if section in sections:
            summary[key], found = parsers[section](sections[section])
            violations.extend(found)
    _parse_optional_sections(sections, summary, violations)
    if not ({"gql", "logic"} & summary.keys()):
        violations.append(
            Violation(
                "missing-query-language", "/", "GQL or LOGIC section is required."
            )
        )
    return receipt(
        subject_kind="search-document",
        raw_input=document.encode(),
        state_digest=state_digest,
        parsed_summary=summary,
        violations=violations,
    )


def _parse_optional_sections(sections, summary, violations) -> None:
    if "TURBO" in sections:
        words = sections["TURBO"].split()
        if len(words) != 2 or words[0] != "USE":
            violations.append(
                Violation(
                    "invalid-turbo-section", "/TURBO", "Expected USE <profile-id>."
                )
            )
        else:
            summary["turbo"] = {"profileId": words[1]}
    if "EMIT" in sections:
        emit = sections["EMIT"].strip()
        if emit not in EMIT_KINDS:
            violations.append(
                Violation("invalid-emit-kind", "/EMIT", "EMIT kind is not admitted.")
            )
        else:
            summary["emit"] = emit.lower()
