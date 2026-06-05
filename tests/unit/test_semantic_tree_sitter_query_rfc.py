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


def test_tree_sitter_query_rfc_names_native_projection_boundary_and_next_layers() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "* Current v1 boundary",
        "tree-sitter-compatible native projection",
        "not a claim that every provider links a complete tree-sitter runtime",
        "sourceAuthority = native-parser-adapter",
        "adapterMode = native-projection",
        "* Next projection layers",
        "** Pattern graph plan",
        "** Multi-capture match model",
        "** Field structural selectors",
        "SyntaxQueryMatch",
        "capture-to-capture predicates",
        "fields become structural selectors",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_tree_sitter_query_rfc_names_codeql_as_optional_backend() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "** Execution backend",
        "=executionBackend=",
        "=codeql=",
        "optional semantic backend",
        "=flow-lite=",
        "local source/sink/path frontier",
        "must not require every project",
        "=codeql-query=",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_schema_readme_records_v1_projection_boundary() -> None:
    text = _SCHEMA_README_PATH.read_text(encoding="utf-8")

    required_terms = [
        "current v1 implementation boundary",
        "tree-sitter-compatible native projection",
        "`sourceAuthority=native-parser-adapter`",
        "`adapterMode=native-projection`",
        "ASP-compiled pattern graphs",
        "multi-capture match rows",
        "field structural selectors",
        "`executionBackend=codeql`",
        "`adapterMode=codeql-query`",
        "optional semantic backend",
        "`flow-lite` local source/sink/path frontier",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []
