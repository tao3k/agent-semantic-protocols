"""Contract checks for the semantic query projection RFC."""

from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = (
    _REPO_ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.10-semantic-query-projection-protocol.org"
)


def test_projection_rfc_names_schema_and_reverse_navigation_contract() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "schemas/semantic-query-packet.v1.schema.json",
        "=matches[].projection=",
        "=projection.nodes[]=",
        "=projection.renderedNodeIds=",
        "=projection.omitted[]=",
        "=projection.expandActions[]=",
        "=projection.exactRead=",
        "=projection.sourceFingerprint=",
        "test_semantic_query_packet_projection_uniqueness.py",
        "parser-compact-token-cost.v1",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_schema_readme_points_to_projection_rfc() -> None:
    text = (_REPO_ROOT / "schemas" / "README.md").read_text(encoding="utf-8")

    assert "docs/10-19-rfcs/10.10-semantic-query-projection-protocol.org" in text
