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
}

fn prepare_source_index_bench_db(
    root: &Path,
) -> agent_semantic_content_identity::SourceSnapshotEvidence {
    fs::create_dir_all(root.join(".git")).expect("create project marker");
    let cache_root = project_client_cache_dir(root).expect("client cache dir");
    fs::create_dir_all(&cache_root).expect("create client cache dir");
    let source_snapshot = agent_semantic_content_identity::SourceSnapshotEvidence {
        schema_id: agent_semantic_content_identity::SOURCE_SNAPSHOT_SCHEMA_ID.to_string(),
        algorithm: agent_semantic_content_identity::SOURCE_SNAPSHOT_ALGORITHM.to_string(),
        root_digest: "0".repeat(64),
        source_kind: agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        leaf_count: 512,
        base_root_digest: None,
        provider_digest: "1".repeat(64),
        dirty_paths_digest: None,
    };
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
        file_hashes: vec![ClientCacheFileHash {
            path: "Cargo.toml".to_string(),
            sha256: "0".repeat(64),
            byte_len: 0,
            mtime_ms: 0,
        }],
        owners,
        selectors: Vec::new(),
    }
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
