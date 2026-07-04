"""Project concrete semantic-search-packet fixtures into Lean proof facts."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .semantic_search_packet_fixture_fields import packet_selector_fields
from .semantic_search_packet_fixture_lean import (
    candidate_proof_body,
    render_fixture_lean,
)


@dataclass(frozen=True)
class PacketProjection:
    source_kind: str
    source_packet: str
    identity_kind: str
    contract_valid: bool
    facts: list[dict[str, str]]
    formal_lean: str
    candidate_lean: str


def build_semantic_search_packet_fixture_projection(
    packet_path: Path,
    source_kind: str = "contract-fixture",
) -> PacketProjection:
    packet = json.loads(packet_path.read_text(encoding="utf-8"))
    facts, has_executable_selector, has_display_locator = _packet_facts(packet)
    contract_valid = has_executable_selector
    identity_kind = _identity_kind(has_executable_selector, has_display_locator)

    facts.extend(
        [
            {
                "id": f"{packet_path.stem}.identityKind",
                "path": "$",
                "role": "identity-kind",
                "value": identity_kind,
                "meaning": "Projected packet identity class used by the proof obligation.",
            },
            {
                "id": f"{packet_path.stem}.contractValid",
                "path": "$",
                "role": "contract-valid",
                "value": _json_bool(contract_valid),
                "meaning": "True only when the packet exposes executable selector identity.",
            },
        ]
    )

    return PacketProjection(
        source_kind=source_kind,
        source_packet=str(packet_path),
        identity_kind=identity_kind,
        contract_valid=contract_valid,
        facts=facts,
        formal_lean=render_fixture_lean(
            packet_path.stem,
            has_executable_selector,
            has_display_locator,
            contract_valid,
            proof_body="by\n  sorry",
        ),
        candidate_lean=render_fixture_lean(
            packet_path.stem,
            has_executable_selector,
            has_display_locator,
            contract_valid,
            proof_body=candidate_proof_body(contract_valid),
        ),
    )


def _packet_facts(packet: dict[str, Any]) -> tuple[list[dict[str, str]], bool, bool]:
    facts: list[dict[str, str]] = []
    has_executable_selector = False
    has_display_locator = False

    for owner, field, role, path, present in packet_selector_fields(packet):
        if not present:
            continue
        if role == "executable-selector":
            has_executable_selector = True
        if role == "display-only":
            has_display_locator = True
        facts.append(
            {
                "id": f"{owner}.{field}",
                "path": path,
                "role": role,
                "value": "present",
                "meaning": _fact_meaning(role),
            }
        )

    return facts, has_executable_selector, has_display_locator


def _identity_kind(has_executable_selector: bool, has_display_locator: bool) -> str:
    if has_executable_selector:
        return "selector-owned"
    if has_display_locator:
        return "path-line-only"
    return "missing"


def _fact_meaning(role: str) -> str:
    if role == "executable-selector":
        return "Executable selector identity that may drive source reads."
    return "Display-only locator that must not become executable identity."


def _json_bool(value: bool) -> str:
    return "true" if value else "false"
