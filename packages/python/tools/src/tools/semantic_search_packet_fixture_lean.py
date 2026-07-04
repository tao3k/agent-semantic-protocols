"""Render Lean projection facts for semantic-search-packet sources."""

from __future__ import annotations

import re


def render_fixture_lean(
    packet_name: str,
    has_executable_selector: bool,
    has_display_locator: bool,
    contract_valid: bool,
    proof_body: str,
) -> str:
    packet_id = _lean_identifier(packet_name)
    prefix = f"fixture_{packet_id}"
    theorem_name = (
        f"{prefix}_accepts_selector_identity"
        if contract_valid
        else f"{prefix}_rejects_path_line_fallback"
        if has_display_locator
        else f"{prefix}_rejects_missing_identity"
    )
    proposition = _projection_proposition(prefix, contract_valid, has_display_locator)
    return "\n".join(
        [
            f"def {prefix}_hasExecutableSelector : Bool := {_lean_bool(has_executable_selector)}",
            f"def {prefix}_hasDisplayOnlyLocator : Bool := {_lean_bool(has_display_locator)}",
            f"def {prefix}_contractValid : Bool := {_lean_bool(contract_valid)}",
            "",
            f"theorem {theorem_name} :",
            f"  {proposition} := {proof_body}",
            "",
        ]
    )


def candidate_proof_body(contract_valid: bool) -> str:
    if contract_valid:
        return "by\n  exact And.intro rfl rfl"
    return "by\n  exact And.intro rfl (And.intro rfl rfl)"


def _projection_proposition(
    prefix: str, contract_valid: bool, has_display_locator: bool
) -> str:
    if contract_valid:
        return (
            f"{prefix}_hasExecutableSelector = true ∧\n"
            f"  {prefix}_contractValid = true"
        )
    return (
        f"{prefix}_hasExecutableSelector = false ∧\n"
        f"  {prefix}_hasDisplayOnlyLocator = {_lean_bool(has_display_locator)} ∧\n"
        f"  {prefix}_contractValid = false"
    )


def _lean_identifier(value: str) -> str:
    ident = re.sub(r"[^A-Za-z0-9_]", "_", value)
    ident = re.sub(r"_+", "_", ident).strip("_")
    return ident or "packet_fixture"


def _lean_bool(value: bool) -> str:
    return "true" if value else "false"
