"""Contract checks for the semantic tree-sitter query RFC."""

from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = _REPO_ROOT / "rfcs" / "semantic-tree-sitter-query-protocol.org"
_SCHEMA_README_PATH = _REPO_ROOT / "schemas" / "README.md"


def test_tree_sitter_query_rfc_defines_frontier_code_render_contract() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "** Agent-facing render contract",
        "=compact-graph-frontier=",
        "=corpus-locator=",
        "ASP-compiled plan",
        "ASP-binary embedded catalog copy",
        "internal =--asp-syntax-query-*= plan facts",
        "default non-JSON output",
        "must write only the selected code",
        "=native-projection=",
        "=nativeFactRefs=",
        "=rawSourceStored=false=",
        "=nodeType=/=field=",
        "=nativeNodeType=",
        "matches",
        "shown",
        "omitted",
        "tools/validate-tree-sitter-frontier-code-contract.sh",
        "tools/validate-language-tree-sitter-runtime-boundary.sh",
        "tools/validate-search-read-plan-frontier-contract.sh",
        "python -m tools tree-sitter validate json-abi-corpus",
        "honest native-projection/no-runtime behavior",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_schema_readme_names_query_render_profiles_without_new_packet_surface() -> None:
    text = _SCHEMA_README_PATH.read_text(encoding="utf-8")

    required_terms = [
        "non-`--code` output is locator/frontier evidence only",
        "`--code`",
        "prints pure source code",
        "`compact-graph-frontier` profile",
        "`corpus-locator` profile",
        "ASP-compiled tree-sitter query plan",
        "provider-native projection",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []
