"""Regression coverage for missing-identity Lean packet projections."""

from tools.semantic_search_packet_fixture_lean import (
    candidate_proof_body,
    render_fixture_lean,
)


def test_missing_identity_projection_keeps_display_locator_false() -> None:
    lean_source = render_fixture_lean(
        "rust-search-workspace-packet",
        has_executable_selector=False,
        has_display_locator=False,
        contract_valid=False,
        proof_body=candidate_proof_body(contract_valid=False),
    )

    assert "rejects_missing_identity" in lean_source
    assert "hasDisplayOnlyLocator : Bool := false" in lean_source
    assert "hasDisplayOnlyLocator = false" in lean_source
    assert "rejects_path_line_fallback" not in lean_source


def test_path_line_projection_keeps_path_line_theorem_name() -> None:
    lean_source = render_fixture_lean(
        "bad-path-line-identity-packet",
        has_executable_selector=False,
        has_display_locator=True,
        contract_valid=False,
        proof_body=candidate_proof_body(contract_valid=False),
    )

    assert "rejects_path_line_fallback" in lean_source
    assert "hasDisplayOnlyLocator : Bool := true" in lean_source
    assert "hasDisplayOnlyLocator = true" in lean_source
