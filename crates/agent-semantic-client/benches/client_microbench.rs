use agent_semantic_client::{LanguageId, lookup_source_index_for_language};
use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, ClientMethod, ClientRequest, ProviderId,
    SemanticSchemaId, SemanticSchemaVersion, project_client_cache_dir,
};
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexImport, ClientDbSourceIndexOwner, ClientDbSourceIndexPath,
    ClientDbSourceIndexQueryKey, ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSource,
};
use criterion::{Criterion, criterion_group, criterion_main};
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn client_request_hot_path(c: &mut Criterion) {
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "lexical".to_string(),
        "cache replay".to_string(),
        "--view=seeds".to_string(),
        ".".to_string(),
    ]);
    c.bench_function("client_request_hot_path", |b| {
        b.iter(|| {
            black_box(request.forwarded_args.len());
            black_box(&request);
        });
    });
}

fn source_index_lookup_hot_path(c: &mut Criterion) {
    let root = source_index_bench_root();
    let source_snapshot = prepare_source_index_bench_db(&root);
    let language_id = LanguageId::from("rust");
    c.bench_function("source_index_lookup_hot_path", |b| {
        b.iter(|| {
            let result = lookup_source_index_for_language(
                black_box(&root),
                black_box(&source_snapshot),
                Some(black_box(&language_id)),
                black_box("bench_symbol_255"),
                black_box(8),
            )
            .expect("lookup source index");
            black_box(result.candidates.len());
        });
    });
    let _ = fs::remove_dir_all(root);
    source_index_merkle_db_scenario(c);
}

fn source_index_merkle_db_scenario(c: &mut Criterion) {
    let root = source_index_bench_root();
    let source_snapshot = prepare_source_index_bench_db(&root);
    let cache_root = project_client_cache_dir(&root).expect("client cache dir");
    let language_id = LanguageId::from("rust");
    let base_snapshot = source_index_bench_workspace_snapshot(512);
    let mut revision = 0_u64;

    let mut group = c.benchmark_group("source_index_merkle_db_scenario");
    group.bench_function("cold_refresh_then_exact_lookup", |b| {
        b.iter_batched(
            source_index_bench_root,
            |cold_root| {
                let cold_snapshot = prepare_source_index_bench_db(&cold_root);
                let result = lookup_source_index_for_language(
                    black_box(&cold_root),
                    black_box(&cold_snapshot),
                    Some(black_box(&language_id)),
                    black_box("bench_symbol_255"),
                    black_box(8),
                )
                .expect("cold refresh then exact lookup");
                assert_eq!(result.candidates.len(), 1);
                black_box(result);
                let _ = fs::remove_dir_all(cold_root);
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.bench_function("warm_exact_lookup", |b| {
        b.iter(|| {
            let result = lookup_source_index_for_language(
                black_box(&root),
                black_box(&source_snapshot),
                Some(black_box(&language_id)),
                black_box("bench_symbol_255"),
                black_box(8),
            )
            .expect("warm lookup source index");
            assert_eq!(result.candidates.len(), 1);
            black_box(result);
        });
    });
    group.bench_function("single_leaf_overlay_refresh_then_lookup", |b| {
        b.iter(|| {
            revision += 1;
            let edited_hash = source_index_bench_file_hash(255, revision);
            let edited_snapshot = base_snapshot.with_overlay_delta(
                [("src/owner_255.rs", edited_hash)],
                std::iter::empty::<&str>(),
            );
            let edited_evidence = edited_snapshot.evidence(
                agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
                "1".repeat(64),
            );
            ClientDbEngine::refresh_source_index_import_from_client_dir(
                black_box(&cache_root),
                ClientDbSourceIndexRefreshRequest {
                    import: source_index_bench_import_with_revision(&root, Some((255, revision))),
                    file_count: 512,
                    source_snapshot: edited_evidence.clone(),
                },
            )
            .expect("refresh edited source index");
            let result = lookup_source_index_for_language(
                black_box(&root),
                black_box(&edited_evidence),
                Some(black_box(&language_id)),
                black_box("bench_symbol_255"),
                black_box(8),
            )
            .expect("lookup edited source index");
            assert_eq!(result.candidates.len(), 1);
            black_box((edited_snapshot.root_digest(), result));
        });
    });
    group.finish();

    let _ = fs::remove_dir_all(root);
}

fn source_index_bench_workspace_snapshot(
    file_count: usize,
) -> agent_semantic_content_identity::WorkspaceSnapshot {
    agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes((0..file_count).map(
        |index| {
            (
                format!("src/owner_{index}.rs"),
                source_index_bench_file_hash(index, 0),
            )
        },
    ))
}

fn prepare_source_index_bench_db(
    root: &Path,
) -> agent_semantic_content_identity::SourceSnapshotEvidence {
    fs::create_dir_all(root.join(".git")).expect("create project marker");
    let cache_root = project_client_cache_dir(root).expect("client cache dir");
    fs::create_dir_all(&cache_root).expect("create client cache dir");
    let source_snapshot = source_index_bench_workspace_snapshot(512).evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "1".repeat(64),
    );
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &cache_root,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_bench_import(root),
            file_count: 512,
            source_snapshot: source_snapshot.clone(),
        },
    )
    .expect("replace source index");
    source_snapshot
}

fn source_index_bench_import(root: &Path) -> ClientDbSourceIndexImport {
    source_index_bench_import_with_revision(root, None)
}

fn source_index_bench_import_with_revision(
    root: &Path,
    edited_owner: Option<(usize, u64)>,
) -> ClientDbSourceIndexImport {
    let owners = (0..512)
        .map(|index| ClientDbSourceIndexOwner {
            owner_path: ClientDbSourceIndexPath::new(format!("src/owner_{index}.rs")),
            language_id: Some(LanguageId::from("rust")),
            provider_id: Some(ProviderId::from("rs-harness")),
            source_kind: ClientDbSourceIndexSource::new("file"),
            line_count: Some(24),
            query_keys: vec![
                ClientDbSourceIndexQueryKey::new(format!("bench_symbol_{index}")),
                ClientDbSourceIndexQueryKey::new("shared_dependency_surface"),
            ],
        })
        .collect();
    ClientDbSourceIndexImport {
        generation_id: CacheGenerationId::from("bench-generation"),
        project_root: root.to_path_buf(),
        schema_id: SemanticSchemaId::from("agent.semantic-protocols.source-index"),
        schema_version: SemanticSchemaVersion::from("1"),
        file_hashes: (0..512)
            .map(|index| {
                let revision = edited_owner
                    .filter(|(owner_index, _)| *owner_index == index)
                    .map_or(0, |(_, revision)| revision);
                ClientCacheFileHash {
                    path: format!("src/owner_{index}.rs"),
                    sha256: source_index_bench_file_hash(index, revision),
                    byte_len: 24,
                    mtime_ms: revision,
                }
            })
            .collect(),
        owners,
        selectors: Vec::new(),
    }
}

fn source_index_bench_file_hash(index: usize, revision: u64) -> String {
    blake3::hash(format!("bench-owner-{index}-revision-{revision}").as_bytes())
        .to_hex()
        .to_string()
}

fn source_index_bench_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("agent-client-source-index-bench-{nanos}"))
}

criterion_group!(
    benches,
    client_request_hot_path,
    source_index_lookup_hot_path
);
criterion_main!(benches);
