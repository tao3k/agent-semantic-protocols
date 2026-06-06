"""Checks for RFC 011 real-project evidence docs."""

from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[2]
_RUST_EVIDENCE = (
    _REPO_ROOT
    / "docs"
    / "30-39-research"
    / "31.19-rfc011-rust-syntax-real-evidence.org"
)
_TYPESCRIPT_EVIDENCE = (
    _REPO_ROOT
    / "docs"
    / "30-39-research"
    / "31.20-rfc011-typescript-syntax-real-evidence.org"
)
_BENCHMARK_REVIEW = (
    _REPO_ROOT
    / "docs"
    / "30-39-research"
    / "31.21-rfc011-native-projection-benchmark-review.org"
)
_ROADMAP = (
    _REPO_ROOT
    / "docs"
    / "30-39-research"
    / "31.18-tree-sitter-query-rfc-roadmap.org"
)


def test_rust_syntax_real_evidence_records_bounded_claims() -> None:
    text = _RUST_EVIDENCE.read_text(encoding="utf-8")

    required_terms = [
        "[syntax-real-evidence] language=rust provider=rs-harness project=agent-semantic-protocols",
        "metrics=commandCount=5,providerProcessCount=5,packetBytes=3914,coldElapsedMs=303,warmElapsedMs=93",
        "cacheClaim=warm-provider",
        "does not claim cache hit replay",
        "repo-relative owner selector returned a non-miss frontier",
        "hook recovery need the repo-relative",
        "The source text is intentionally not copied",
        "Warm-provider behavior was observed, but this is not a cache hit",
        "31.21-rfc011-native-projection-benchmark-review.org",
    ]
    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_typescript_syntax_real_evidence_records_bounded_claims() -> None:
    text = _TYPESCRIPT_EVIDENCE.read_text(encoding="utf-8")

    required_terms = [
        "[syntax-real-evidence] language=typescript provider=ts-harness project=typescript-lang-project-harness",
        "metrics=commandCount=6,providerProcessCount=6,packetBytes=13040,coldElapsedMs=31,warmElapsedMs=50",
        "cacheClaim=warm-provider",
        "does not claim cache hit replay",
        "src/parser/native_syntax/tree-sitter-query.ts",
        "renderTypeScriptTreeSitterQuery",
        "The source text is intentionally not copied",
        "provider-activation-missing",
        "this is not a cache hit",
        "31.21-rfc011-native-projection-benchmark-review.org",
    ]
    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_roadmap_records_rust_and_typescript_evidence() -> None:
    text = _ROADMAP.read_text(encoding="utf-8")

    required_terms = [
        "31.19-rfc011-rust-syntax-real-evidence.org",
        "31.20-rfc011-typescript-syntax-real-evidence.org",
        "31.21-rfc011-native-projection-benchmark-review.org",
        "accepts the Rust and TypeScript native-projection v1 lanes",
        "Neither record claims cache hit replay",
    ]
    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_benchmark_review_accepts_only_rust_typescript_native_projection() -> None:
    text = _BENCHMARK_REVIEW.read_text(encoding="utf-8")

    required_terms = [
        "The Rust and TypeScript real-project evidence records satisfy the RFC 011",
        "status=native-projection-v1-reviewed",
        "cacheClaim=warm-provider= is not a cache hit",
        "Python still keeps =pending=real-project-benchmark=",
        "Julia cache payoff remains out of scope",
        "CodeQL backend execution remains out of scope",
        "arbitrary S-expression support remains out of scope",
    ]
    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []
