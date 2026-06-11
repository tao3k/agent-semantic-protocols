"""Contract checks for RFC 012 native relation/flow and CodeQL extension plan."""

from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = (
    _REPO_ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.12-asp-native-relation-flow-codeql.org"
)


def test_asp_relation_flow_rfc_defines_native_first_boundary() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "Native parser authority is the primary path.",
        "Relations are protocol facts, not prompt inference.",
        "Flow is bounded before it is global.",
        "CodeQL is an ASP extension, not a provider default or a command family.",
        "Do not put CodeQL database creation in the hot path",
        "ordinary fuzzy search",
        "owner discovery",
        "hook recovery",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_asp_relation_flow_rfc_defines_catalog_and_packets() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "ASP relation catalog",
        "ASP flow-lite catalog",
        "=semantic-relation-plan.v1.schema.json=",
        "=semantic-flow-lite.v1.schema.json=",
        "=semantic-codeql-evidence.v1.schema.json=",
        "flow-lite -> relation rows -> compact graph frontier -> exact selector --code",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_asp_relation_flow_rfc_defines_flow_lite_query_catalog_contract() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "asp rust query --catalog flow-lite",
        "source.call=payload_string sink.constructs=ToolAction scope.fn=collect_tool_actions",
        "=--catalog flow-lite= is not a tree-sitter =.scm= catalog id.",
        "tree-sitter-compatible syntax catalog branch",
        'key          := "source.call" | "sink.constructs" | "scope.fn"',
        "This is not a general predicate language",
        "=sourceAuthority = native-parser=",
        "=executionBackend = native-parser=",
        "=adapterMode = native-projection=",
        "[query-flow-lite]",
        "semantic-flow-lite.v1",
        "locator/provenance surface",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_asp_relation_flow_rfc_keeps_codeql_optional_and_structured() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "sourceAuthority = codeql",
        "executionBackend = codeql",
        "adapterMode = codeql-query",
        "extension must return a bounded unsupported/backend-unavailable receipt",
        "The prompt must not include raw query output by default.",
        "A provider may consume CodeQL extension evidence only after:",
        "extension descriptor/status receipt",
        "extensionId = codeql",
        "enabled = false",
        "experimental = true",
        "mode = \"disabled\"",
        "allowHotPath = false",
        "=extensions.codeql.enabled=true=",
        "ASP extension adapter",
        "asp extension codeql status .",
        "asp evidence codeql --profile global-flow --cache-only .",
        "=next=warm-cache=",
        "=avoid=run-codeql-in-agent-hot-path=",
        "=backend-unavailable= CodeQL evidence artifact with =rowCount = 0=",
        "without advertising =codeql=",
        "=sandtables/rust/codeql-backend-unavailable-flow.json=",
        "=sandtables/rust/codeql-cli-metadata-flow.json=",
        "=sandtables/rust/codeql-bounded-source-file-flow.json=",
        "=codeql= term native-search path",
        "ASP_RUN_SLOW_CODEQL=1",
        "codeql_bounded_evidence.py",
        "codeql database create",
        "codeql query run",
        "raw-dbscheme",
        "codeql:file:src/lib.rs",
        "codeql/rust-all",
        "databaseCacheStatus",
        ".cache/agent-semantic-protocol/codeql-fixtures",
        "explicit user opt-in for a slow extension evidence command",
        "precomputed cache/artifact hit",
        "CI, nightly, or development-mode evidence production",
        "not a replacement for",
        "codeql resolve languages --format=json",
        "metadata-only",
        "provider CodeQL",
        "hot-path backend",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []
