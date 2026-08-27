"""Contract checks for the canonical ASP Server-first RFC."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RFC = ROOT / "docs" / "10-19-rfcs" / "10.05-interactive-graph-first-progressive-searchloop.org"
REMOVED_STEM = "10.05-" + "cli-" + "first-harness-ux"
LEGACY_ROOT = ROOT / "docs" / "10-19-rfcs" / f"{REMOVED_STEM}.org"
LEGACY_TREE = ROOT / "docs" / "10-19-rfcs" / REMOVED_STEM


def test_server_first_rfc_is_the_only_10_05_architecture_root() -> None:
    text = RFC.read_text(encoding="utf-8")
    required = [
        "ASP Server-First Agent Search Architecture",
        "public gRPC ClientFrame",
        "thin typed CLI adapter",
        "Git history is the only historical",
    ]
    assert [term for term in required if term not in text] == []


def test_cli_first_rfc_tree_is_deleted() -> None:
    assert not LEGACY_ROOT.exists()
    assert not LEGACY_TREE.exists()
