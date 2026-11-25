// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! High-performance multi-scale Criterion benchmarks for ASP's converged pure-Rust
//! search engine hot paths:
//! 1. `rank_source_index_report` across 100 / 1,000 / 5,000 candidates
//! 2. `admit_search_plan` multi-axis progressive playbook admission
//! 3. `publish_generation_artifact` + `load_active_generation_artifact` CAS path
//! 4. `persist_topology_edges` + `load_topology_edges` generation-scoped custody
//! 5. `SortedRecordTable` binary-search lookup hot path

use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

use agent_semantic_search::{
    InMemorySearchStoragePort, MarlinConvergedSearchEngine, SearchGenerationDigest,
    SearchProjectId, SearchTopologyEdgeId, SearchTopologyEdgeRecord, SearchWorkspaceId,
    SourceIndexRankCandidate, SourceIndexRankRequest, WorkspaceIndexConfig, WorkspaceSearchIndex,
    WorkspaceSearchQuery, analyze_native_rg_argv, analyze_tantivy_query,
};
use agent_semantic_search_playbook::{
    GraphNativeBlock, SearchPlaybookComposition, SearchPlaybookLeaf, SearchPlaybookPlanInput,
    TopologyOwnerMembershipBlock,
};
use agent_semantic_topology::{
    OrgDocumentProjection, OrgDocumentProjectionPort, OrgOverlayDocumentProjection,
    OrgProjectionPortError,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

struct BenchOrgPort;

impl OrgDocumentProjectionPort for BenchOrgPort {
    fn project_document(
        &self,
        source: &str,
    ) -> Result<OrgDocumentProjection, OrgProjectionPortError> {
        Ok(OrgDocumentProjection::new(
            OrgDocumentProjection::digest_for_source(source),
            Vec::new(),
            Vec::new(),
        ))
    }

    fn inspect_overlay(
        &self,
        source: &str,
    ) -> Result<OrgOverlayDocumentProjection, OrgProjectionPortError> {
        Ok(OrgOverlayDocumentProjection::new(
            source,
            OrgDocumentProjection::digest_for_source(source),
            true,
        ))
    }
}

fn synthetic_source_candidates(count: usize) -> Vec<SourceIndexRankCandidate> {
    let modules = ["search", "topology", "playbook", "tree_sitter", "storage"];
    (0..count)
        .map(|ordinal| {
            let module = modules[ordinal % modules.len()];
            SourceIndexRankCandidate {
                ordinal,
                path: format!("crates/agent-semantic-{module}/src/module_{ordinal:04}.rs"),
                query_keys: vec![
                    format!("fn_rank_{ordinal}"),
                    format!("module_{module}"),
                    if ordinal == count / 2 {
                        "MarlinConvergedSearchEngine".to_owned()
                    } else {
                        format!("symbol_{ordinal}")
                    },
                ],
            }
        })
        .collect()
}

fn bench_source_index_rank(c: &mut Criterion) {
    let engine = MarlinConvergedSearchEngine::new(
        Arc::new(BenchOrgPort),
        Arc::new(InMemorySearchStoragePort::new()),
    );
    let mut group = c.benchmark_group("asp_source_index_rank");

    for count in [100_usize, 1_000, 5_000] {
        let candidates = synthetic_source_candidates(count);
        let request = SourceIndexRankRequest {
            query: "crates/agent-semantic-search MarlinConvergedSearchEngine".to_owned(),
            candidates,
        };
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("rank_candidates", count),
            &request,
            |bencher, req| {
                bencher.iter(|| {
                    let report = engine.rank_source_index(black_box(req).clone());
                    black_box(report.ranked_candidates.len());
                });
            },
        );
    }

    group.finish();
}

fn bench_playbook_admission(c: &mut Criterion) {
    let engine = MarlinConvergedSearchEngine::new(
        Arc::new(BenchOrgPort),
        Arc::new(InMemorySearchStoragePort::new()),
    );
    let plan = SearchPlaybookPlanInput {
        workspace: Some("marlin-main".to_owned()),
        languages: vec!["rust".to_owned(), "python".to_owned()],
        documents: vec!["org".to_owned()],
        composition: SearchPlaybookComposition::Chain(vec![
            SearchPlaybookComposition::Intersect(vec![
                SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Rg(vec![
                    "-n".to_owned(),
                    "MarlinConvergedSearchEngine".to_owned(),
                ])),
                SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Tantivy(vec![
                    "title:\"converged search\"^2 OR body:SearchEngineStoragePort".to_owned(),
                ])),
                SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Topology(
                    TopologyOwnerMembershipBlock {
                        kind: "file".to_owned(),
                        exact_path: None,
                        path_prefix: Some("crates/".to_owned()),
                        extension: Some("rs".to_owned()),
                        path_glob: None,
                    },
                )),
            ]),
            SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Graph(GraphNativeBlock {
                language: "pgql".to_owned(),
                argv: vec!["MATCH (a)-[r]->(b) RETURN a, r, b".to_owned()],
            })),
        ]),
    };

    let mut group = c.benchmark_group("asp_playbook_admission");
    group.throughput(Throughput::Elements(1));
    group.bench_function("admit_multi_axis_plan", |bencher| {
        bencher.iter(|| {
            let admitted = engine
                .admit_search_plan(black_box(&plan).clone())
                .expect("valid playbook plan");
            black_box(admitted);
        });
    });
    group.finish();
}

fn bench_generation_cas_and_edges(c: &mut Criterion) {
    let mut cas_group = c.benchmark_group("asp_generation_cas_storage");
    for payload_bytes in [1_024_usize, 16_384, 65_536] {
        let payload = vec![0x5Au8; payload_bytes];
        cas_group.throughput(Throughput::Bytes(payload_bytes as u64));
        cas_group.bench_with_input(
            BenchmarkId::new("publish_and_load", format!("{payload_bytes}B")),
            &payload,
            |bencher, body| {
                let engine = MarlinConvergedSearchEngine::new(
                    Arc::new(BenchOrgPort),
                    Arc::new(InMemorySearchStoragePort::new()),
                );
                let project = SearchProjectId::new("asp-bench").expect("project id");
                let workspace = SearchWorkspaceId::new("ws-main").expect("workspace id");
                let gen_digest = SearchGenerationDigest::new(
                    "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("generation digest");
                let mut current_hash = None;

                bencher.iter(|| {
                    let record = engine
                        .publish_generation_artifact(
                            project.clone(),
                            workspace.clone(),
                            current_hash.as_ref(),
                            gen_digest.clone(),
                            "application/octet-stream",
                            black_box(body).clone(),
                        )
                        .expect("CAS publish");
                    current_hash = Some(record.artifact_hash.clone());
                    let loaded = engine
                        .load_active_generation_artifact(&project, &workspace)
                        .expect("load active")
                        .expect("present");
                    black_box(loaded);
                });
            },
        );
    }
    cas_group.finish();

    let mut edge_group = c.benchmark_group("asp_topology_edge_custody");
    for edge_count in [32_usize, 256, 1_024] {
        let project = SearchProjectId::new("asp-bench").expect("project id");
        let gen_digest = SearchGenerationDigest::new(
            "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .expect("generation digest");
        let edges: Vec<SearchTopologyEdgeRecord> = (0..edge_count)
            .map(|idx| SearchTopologyEdgeRecord {
                project_id: project.clone(),
                generation_digest: gen_digest.clone(),
                edge_id: SearchTopologyEdgeId::new(format!("edge-{idx:04}")).expect("edge id"),
                from_node: format!("owner:crates/pkg_{}/src/lib.rs", idx % 32),
                to_node: format!("owner:crates/pkg_{}/src/search.rs", (idx + 1) % 32),
                relation: "calls".to_owned(),
                payload: b"edge-payload".to_vec(),
            })
            .collect();

        edge_group.throughput(Throughput::Elements(edge_count as u64));
        edge_group.bench_with_input(
            BenchmarkId::new("persist_and_load", edge_count),
            &edges,
            |bencher, batch| {
                bencher.iter(|| {
                    let engine = MarlinConvergedSearchEngine::new(
                        Arc::new(BenchOrgPort),
                        Arc::new(InMemorySearchStoragePort::new()),
                    );
                    let inserted = engine
                        .persist_topology_edges(black_box(batch))
                        .expect("persist edges");
                    let loaded = engine
                        .load_topology_edges(&project, &gen_digest)
                        .expect("load edges");
                    black_box((inserted, loaded.len()));
                });
            },
        );
    }
    edge_group.finish();
}

fn bench_query_analysis_hot_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("asp_query_analysis");
    let tantivy_query = "title:\"MarlinConvergedSearchEngine\"^2 AND (body:SearchEngineStoragePort OR path:crates/agent-semantic-search)";
    let rg_argv = vec![
        "-n".to_owned(),
        "-S".to_owned(),
        "MarlinConvergedSearchEngine".to_owned(),
    ];

    group.throughput(Throughput::Elements(1));
    group.bench_function("analyze_tantivy_query", |bencher| {
        bencher.iter(|| {
            let analysis = analyze_tantivy_query(black_box(tantivy_query));
            black_box(analysis);
        });
    });
    group.bench_function("analyze_native_rg_argv", |bencher| {
        bencher.iter(|| {
            let analysis = analyze_native_rg_argv(black_box(&rg_argv));
            black_box(analysis);
        });
    });
    group.finish();
}

fn bench_real_corpus_playbook_execution(c: &mut Criterion) {
    let engine = MarlinConvergedSearchEngine::new(
        Arc::new(BenchOrgPort),
        Arc::new(InMemorySearchStoragePort::new()),
    );
    let corpus_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent-semantic-search-playbook/src");
    let plan = SearchPlaybookPlanInput {
        workspace: Some("asp-real-corpus".to_owned()),
        languages: vec!["rust".to_owned()],
        documents: Vec::new(),
        composition: SearchPlaybookComposition::Intersect(vec![
            SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Rg(vec![
                "-n".to_owned(),
                "admit_search_playbook_plan".to_owned(),
            ])),
            SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Tantivy(vec![
                "title:\"playbook\"^2 OR body:admit_search_playbook_plan".to_owned(),
            ])),
        ]),
    };
    let admitted = engine
        .admit_search_plan(plan)
        .expect("admit real corpus plan");

    let mut group = c.benchmark_group("asp_real_corpus_playbook_execution");
    group.throughput(Throughput::Elements(1));
    group.bench_function("execute_multi_axis_over_real_rust_corpus", |bencher| {
        bencher.iter(|| {
            let report = engine
                .execute_admitted_playbook(black_box(&corpus_root), black_box(&admitted))
                .expect("execute playbook over real corpus");
            assert!(report.matched_files >= 1);
            black_box(report);
        });
    });
    group.finish();
}

fn bench_converged_resident_4axis_hot_path(c: &mut Criterion) {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("ASP repo root");
    let cfg = WorkspaceIndexConfig {
        skip_dirs: vec![
            "target".into(),
            ".git".into(),
            "node_modules".into(),
            ".devenv".into(),
        ],
        ..WorkspaceIndexConfig::default()
    };
    let index = WorkspaceSearchIndex::build_with_config(repo_root, cfg);
    let rust_lang = vec!["rust".to_owned()];

    let mut group = c.benchmark_group("asp_converged_resident_4axis");
    group.throughput(Throughput::Elements(1));

    group.bench_function("rg_literal_plus_bm25", |bencher| {
        let query = WorkspaceSearchQuery {
            pattern: "admit_search_playbook_plan",
            languages: &rust_lang,
            documents: &[],
            path_prefix: Some("crates/"),
            extension: None,
            max_hits: 32,
            context_radius: 4,
        };
        bencher.iter(|| {
            let report = index.query(black_box(&query));
            assert!(report.matched_files >= 1);
            black_box(report);
        });
    });

    group.bench_function("rg_regex_in_ram", |bencher| {
        let query = WorkspaceSearchQuery {
            pattern: r"re:pub\s+fn\s+execute_admitted_playbook",
            languages: &rust_lang,
            documents: &[],
            path_prefix: Some("crates/"),
            extension: None,
            max_hits: 32,
            context_radius: 4,
        };
        bencher.iter(|| {
            let report = index.query(black_box(&query));
            assert!(report.matched_files >= 1);
            black_box(report);
        });
    });

    group.bench_function("file_locator_and_fuzzy_path", |bencher| {
        let query = WorkspaceSearchQuery {
            pattern: "flcltr",
            languages: &rust_lang,
            documents: &[],
            path_prefix: Some("crates/"),
            extension: None,
            max_hits: 32,
            context_radius: 2,
        };
        bencher.iter(|| {
            let report = index.query(black_box(&query));
            black_box(report);
        });
    });

    group.bench_function("tantivy_fuzzy_typo_tolerance", |bencher| {
        let query = WorkspaceSearchQuery {
            pattern: "playbok",
            languages: &rust_lang,
            documents: &[],
            path_prefix: Some("crates/"),
            extension: None,
            max_hits: 32,
            context_radius: 2,
        };
        bencher.iter(|| {
            let report = index.query(black_box(&query));
            assert!(report.matched_files >= 1);
            black_box(report);
        });
    });

    group.finish();
}

fn benchmark_config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(40)
}

criterion_group! {
    name = benches;
    config = benchmark_config();
    targets =
        bench_source_index_rank,
        bench_playbook_admission,
        bench_generation_cas_and_edges,
        bench_query_analysis_hot_path,
        bench_real_corpus_playbook_execution,
        bench_converged_resident_4axis_hot_path
}
criterion_main!(benches);
