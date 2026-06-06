"""Provider lane and real-project evidence checks for RFC 011."""

from .helpers import RFC_PATH, missing_terms


def test_tree_sitter_query_rfc_defines_provider_conformance_lanes() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "* Provider conformance lanes",
        "** Lane R: Rust reference native projection",
        "renderProfile=compact-graph-frontier",
        "tools/validate-language-tree-sitter-runtime-boundary.sh",
        "** Lane T: TypeScript compiler projection",
        "TypeScript Compiler API, =tsconfig=, and project references",
        "renderProfile=corpus-locator",
        "** Lane P: Python AST/token projection",
        "=ast=, =tokenize=, and =symtable=",
        "indentation-sensitive ownership",
        "** Lane J: JuliaSyntax export and cache replay",
        "ASP cache replay proves repeated syntax queries can render from fresh",
        "** Lane Q: CodeQL or flow-lite semantic backend",
        "renderProfile=flow-lite-frontier",
        "Default syntax guides keep =native-parser= lanes available",
        "** Lane promotion record",
        "[syntax-lane] language=<id> provider=<id> lane=<R|T|P|J|Q>",
    ]

    assert missing_terms(text, required_terms) == []


def test_tree_sitter_query_rfc_resolves_native_projection_v1_decisions() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "#+STATUS: native-projection-v1-closure",
        "* Resolved decisions for native-projection v1",
        "Syntax query artifacts use the dedicated",
        "=semantic-tree-sitter-query/<id>.json=",
        "=semantic-tree-sitter-provenance.v1= is the shared base",
        "Grammar profile versions are provider-owned stable strings",
        "Arbitrary S-expression input is gated per provider through",
        "=queryInputForms=s-expression=",
        "Catalog filenames are =.scm= only",
        "ASP does not accept =.scheme= catalog",
        "Scheme-like S-expression query text is an input form",
        "TypeScript follows Rust as the first compiler/native projection target",
    ]
    forbidden_terms = [
        "=.scheme= may be accepted as a compatibility spelling",
        "Whether ASP should accept =.scheme= catalog filenames",
    ]

    assert missing_terms(text, required_terms) == []
    assert missing_terms(text, forbidden_terms) == forbidden_terms


def test_tree_sitter_query_rfc_records_current_native_projection_lanes() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "Current native-projection v1 lane records:",
        "[syntax-lane] language=rust provider=rs-harness lane=R",
        "queryInputForms=selector,code-shaped,catalog-id,s-expression",
        "renderProfile=compact-graph-frontier",
        "[syntax-lane] language=typescript provider=ts-harness lane=T",
        "renderProfile=corpus-locator",
        "[syntax-lane] language=python provider=py-harness lane=P",
        "pending=real-project-benchmark",
        "do not claim cache replay speedup",
        "arbitrary S-expression support for TypeScript/Python",
        "CodeQL backend",
    ]

    assert missing_terms(text, required_terms) == []


def test_tree_sitter_query_rfc_defines_real_project_evidence_gate() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "** Native-projection v1 real-project evidence gate",
        "representative Rust workspace",
        "representative TypeScript workspace",
        "[syntax-real-evidence] language=<id> provider=<id> project=<name>",
        "commands=search-prime,syntax-frontier,exact-selector-code,hook-recovery",
        "metrics=commandCount=<n>,providerProcessCount=<n>,packetBytes=<n>,coldElapsedMs=<n>,warmElapsedMs=<n>",
        "metrics=syntaxQueryCount=<n>,exactCodeCount=<n>,manualRangeScanCount=<n>,repeatedTriggerReduction=<n>",
        "outputs=frontier-no-code,pure-code-stdout,registry-descriptor,query-corpus",
        "cacheClaim=none|warm-provider|hit",
        "=tools/record-syntax-real-evidence.py=",
        "not a packet schema and not a benchmark",
        "rejects =cacheClaim=hit=",
        "Search/read-plan recovery remain discovery surfaces",
        "must print only source text",
        "warm-provider= is not a cache hit",
        "Julia cache payoff and CodeQL backend execution remain out of scope",
    ]

    assert missing_terms(text, required_terms) == []
