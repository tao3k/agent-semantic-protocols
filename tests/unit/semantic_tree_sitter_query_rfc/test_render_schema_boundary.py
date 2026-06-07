"""Render and schema boundary checks for RFC 011."""

from .helpers import RFC_PATH, SCHEMA_README_PATH, missing_terms


def test_tree_sitter_query_rfc_defines_frontier_code_render_contract() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

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
        "python -m tools tree-sitter validate frontier-code-contract",
        "python -m tools tree-sitter validate runtime-boundary",
        "python -m tools tree-sitter validate search-read-plan-frontier-contract",
        "python -m tools tree-sitter validate json-abi-corpus",
        "honest native-projection/no-runtime behavior",
    ]

    assert missing_terms(text, required_terms) == []


def test_schema_readme_names_query_render_profiles_without_new_packet_surface() -> None:
    text = SCHEMA_README_PATH.read_text(encoding="utf-8")

    required_terms = [
        "non-`--code` output is locator/frontier evidence only",
        "`--code`",
        "prints pure source code",
        "`compact-graph-frontier` profile",
        "`corpus-locator` profile",
        "ASP-compiled tree-sitter query plan",
        "provider-native projection",
    ]

    assert missing_terms(text, required_terms) == []


def test_tree_sitter_query_rfc_names_native_projection_boundary_and_next_layers() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

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

    assert missing_terms(text, required_terms) == []


def test_schema_readme_records_v1_projection_boundary() -> None:
    text = SCHEMA_README_PATH.read_text(encoding="utf-8")

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

    assert missing_terms(text, required_terms) == []
