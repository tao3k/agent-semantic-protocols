"""CodeQL and acceptance checks for RFC 011."""

from .helpers import RFC_PATH, missing_terms


def test_tree_sitter_query_rfc_names_codeql_as_optional_backend() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "** Execution backend",
        "=executionBackend=",
        "=codeql=",
        "optional semantic backend",
        "=flow-lite=",
        "local source/sink/path frontier",
        "must not require every project",
        "=codeql-query=",
        "=executionBackends=",
        "must not advertise =codeql=",
    ]

    assert missing_terms(text, required_terms) == []


def test_tree_sitter_query_rfc_defines_acceptance_matrix() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "* Acceptance matrix",
        "V1 accepted",
        "V2/deferred",
        "ASP-compiled flat plan facts",
        "ASP-compiled pattern graph",
        "Single-capture rows",
        "Multi-capture match rows",
        "Fields as required structural selectors",
        "** Promotion rule",
        "RFC -> shared schema -> registry descriptor -> provider projection ->",
        "** Explicit non-goals for v1",
        "Full tree-sitter runtime parity across all providers",
        "Edit-safety decisions from tree-sitter captures alone",
    ]

    assert missing_terms(text, required_terms) == []


def test_tree_sitter_query_rfc_defines_closure_gates() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "* RFC closure gates",
        "** Schema and docs gates",
        "** Provider advertisement gates",
        "** Prompt and cache gates",
        "** Evidence gates",
        "tree-sitter-compatible native projection",
        "=executionBackends=",
        "A provider may list =codeql= only when a real CodeQL-backed executor emits",
        "RFC 012 owns the native relation catalog",
        "pattern graph",
        "multi-capture",
        "field structural selector",
        "Prompt-noise ban: without source windows, cache ids, DB paths, receipt JSON, or artifact ids",
        "prints pure source code and rejects broad multi-match code extraction",
        "Cache replay key: query AST/ABI, selector identity, grammar/profile/catalog fingerprint, execution backend, and file freshness",
        "At least one real-project benchmark records command count",
    ]

    assert missing_terms(text, required_terms) == []
