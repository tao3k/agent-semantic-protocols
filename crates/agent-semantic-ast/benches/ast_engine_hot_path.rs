// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Reproducible Criterion microbenchmarks for `agent-semantic-ast` pure Rust
//! Backend Runtime hot paths (RFC 10.35): Provider Registry routing, 1000-LOC
//! lossless span splicing, and Frontier 2 bounded calibration.

use std::fmt::Write as _;
use std::hint::black_box;
use std::sync::Arc;

use agent_semantic_ast::{
    AstEngineError, AstInspectRequest, AstInspectResponse, AstLanguageId, AstNodeLocator,
    AstProviderCapabilities, AstProviderDescriptor, AstProviderId, AstProviderPort,
    AstProviderRegistry, AstProviderTransportKind, AstSyntaxDiagnostic, AstVerifyCandidateResult,
    AstVerifyRequest, AstVerifyResponse, AstWorkspacePath, ByteSpan, CalibrationRequest,
    LexicalDelimiterProfile, SpanReplacement, blake3_source_digest, calibrate_with_oracle,
    splice_lossless,
};
use criterion::{Criterion, criterion_group, criterion_main};

struct BenchProvider {
    pid: AstProviderId,
    lid: AstLanguageId,
    profile: LexicalDelimiterProfile,
}

impl AstProviderPort for BenchProvider {
    fn provider_id(&self) -> &AstProviderId {
        &self.pid
    }
    fn language_id(&self) -> &AstLanguageId {
        &self.lid
    }
    fn lexical_profile(&self) -> LexicalDelimiterProfile {
        self.profile.clone()
    }
    fn inspect(&self, _req: &AstInspectRequest) -> Result<AstInspectResponse, AstEngineError> {
        Ok(AstInspectResponse {
            accepted: true,
            nodes: Vec::new(),
            edges: Vec::new(),
            diagnostics: Vec::new(),
            syntax_surface: None,
        })
    }
    fn verify(&self, req: &AstVerifyRequest) -> Result<AstVerifyResponse, AstEngineError> {
        let results = req
            .candidates
            .iter()
            .map(|c| {
                let open = c.source_utf8.bytes().filter(|&b| b == b'{').count();
                let close = c.source_utf8.bytes().filter(|&b| b == b'}').count();
                let accepted = open == close;
                AstVerifyCandidateResult {
                    candidate_id: c.candidate_id.clone(),
                    accepted,
                    diagnostics: if accepted {
                        Vec::new()
                    } else {
                        vec![AstSyntaxDiagnostic {
                            byte_start: c.source_utf8.len().saturating_sub(1),
                            byte_end: c.source_utf8.len(),
                            message: "unclosed `{`".to_owned(),
                            expected_tokens: vec!["}".to_owned()],
                        }]
                    },
                }
            })
            .collect();
        Ok(AstVerifyResponse { results })
    }
}

fn build_five_language_registry() -> AstProviderRegistry {
    let mut registry = AstProviderRegistry::new();
    let specs = [
        ("asp-rust", "rust", ".rs", LexicalDelimiterProfile::rust()),
        (
            "asp-typescript",
            "typescript",
            ".ts",
            LexicalDelimiterProfile::typescript(),
        ),
        (
            "asp-python",
            "python",
            ".py",
            LexicalDelimiterProfile::python(),
        ),
        (
            "asp-julia",
            "julia",
            ".jl",
            LexicalDelimiterProfile::python(),
        ),
        (
            "asp-gerbil-scheme",
            "scheme",
            ".ss",
            LexicalDelimiterProfile::scheme(),
        ),
    ];
    for (pid, lid, ext, profile) in specs {
        let pid = AstProviderId::new(pid).expect("pid");
        let lid = AstLanguageId::new(lid).expect("lid");
        let port = Arc::new(BenchProvider {
            pid: pid.clone(),
            lid: lid.clone(),
            profile: profile.clone(),
        });
        registry
            .register(
                AstProviderDescriptor {
                    provider_id: pid,
                    language_id: lid,
                    source_extensions: vec![ext.to_owned()],
                    lexical_profile: profile,
                    capabilities: AstProviderCapabilities::default(),
                    transport_kind: AstProviderTransportKind::HttpJsonLoopback,
                    endpoint: None,
                },
                port,
            )
            .expect("register provider");
    }
    registry
}

fn synthetic_1000_loc_source() -> (String, ByteSpan) {
    let mut src = String::with_capacity(48_000);
    let mut target_span = ByteSpan { start: 0, end: 0 };
    for idx in 0..200 {
        let start = src.len();
        let _ = writeln!(
            src,
            "// Preserved trivia comment #{idx}\nfn item_{idx}(x: i64) -> i64 {{\n    let y = x + {idx};\n    y * 2\n}}"
        );
        if idx == 120 {
            target_span = ByteSpan {
                start,
                end: src.len().saturating_sub(1),
            };
        }
    }
    (src, target_span)
}

fn bench_ast_engine_hot_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("asp_ast_engine_hot_path");

    let registry = build_five_language_registry();
    let sample_path = AstWorkspacePath::new("crates/engine/src/transaction.rs").expect("path");
    group.bench_function("registry_resolve_5_providers_by_path", |b| {
        b.iter(|| {
            black_box(
                registry
                    .resolve_for_path(black_box(&sample_path))
                    .expect("ok"),
            )
        })
    });

    let (src_1000, target_span) = synthetic_1000_loc_source();
    let edits = vec![SpanReplacement {
        span: target_span,
        replacement: "fn item_120(x: i64) -> i64 {\n    x.saturating_mul(4)\n}".to_owned(),
    }];
    group.bench_function("lossless_span_splice_1000_loc", |b| {
        b.iter(|| {
            black_box(splice_lossless(black_box(&src_1000), black_box(&edits)).expect("splice ok"))
        })
    });

    let provider = BenchProvider {
        pid: AstProviderId::new("asp-rust").expect("pid"),
        lid: AstLanguageId::new("rust").expect("lid"),
        profile: LexicalDelimiterProfile::rust(),
    };
    let broken_src = "fn compute(x: i64) -> i64 {\n    let v = x + 1;\n    v * 2\n";
    let preimage = blake3_source_digest(broken_src.as_bytes());
    let cal_req = CalibrationRequest {
        path: &sample_path,
        preimage_hash: &preimage,
        raw_source: broken_src,
        edited_span: ByteSpan {
            start: 0,
            end: broken_src.len(),
        },
        locator: Some(AstNodeLocator::new("src/transaction.rs#fn:compute").expect("loc")),
        allow_auto_calibrate: true,
    };
    group.bench_function("frontier2_calibrate_missing_brace", |b| {
        b.iter(|| black_box(calibrate_with_oracle(&provider, black_box(&cal_req)).expect("cal ok")))
    });

    group.finish();
}

criterion_group!(benches, bench_ast_engine_hot_paths);
criterion_main!(benches);
