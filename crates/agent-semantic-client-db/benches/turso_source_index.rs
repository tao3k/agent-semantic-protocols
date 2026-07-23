use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbEngine, ClientDbSourceIndexImport,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexMembershipChangeSet, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexSource, build_source_index_import,
};
use agent_semantic_content_identity::{SourceSnapshotKind, WorkspaceSnapshot};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

fn temp_client_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-client-db-bench-source-index-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn hex_digest(index: usize) -> String {
    format!("{:064x}", index + 1)
}

fn source_index_import(
    project_root: &Path,
    generation_id: &str,
    file_count: usize,
) -> ClientDbSourceIndexImport {
    build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from(generation_id),
        project_root: project_root.to_path_buf(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: (0..file_count)
            .map(|index| ClientCacheFileHash {
                path: format!("src/file_{index:04}.rs"),
                sha256: hex_digest(index),
                byte_len: 64,
                mtime_ms: index as u64,
            })
            .collect(),
        files: (0..file_count)
            .map(|index| {
                let symbol = format!("bench_symbol_{index:04}");
                ClientDbSourceIndexImportFile {
                    relative_path: format!("src/file_{index:04}.rs"),
                    language_id: LanguageId::from("rust"),
                    provider_id: ProviderId::from("rs-harness"),
                    text: format!("pub fn {symbol}() -> usize {{ {index} }}\n"),
                    selectors: Vec::new(),
                }
            })
            .collect(),
    })
    .expect("build source-index benchmark import")
}

fn refresh_request(
    project_root: &Path,
    generation_id: &str,
    file_count: usize,
) -> ClientDbSourceIndexRefreshRequest {
    let snapshot = WorkspaceSnapshot::from_file_hashes(
        (0..file_count).map(|index| (format!("src/file_{index:04}.rs"), hex_digest(index))),
    );
    let source_snapshot = snapshot.evidence(SourceSnapshotKind::Filesystem, "d".repeat(64));
    ClientDbSourceIndexRefreshRequest {
        import: source_index_import(project_root, generation_id, file_count),
        file_count: file_count.min(u32::MAX as usize) as u32,
        source_snapshot,
        membership_change_set: ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
    }
}

fn turso_source_index(c: &mut Criterion) {
    let project_root = temp_client_dir("project-root");
    let mut group = c.benchmark_group("client_db_turso_source_index");
    group.sample_size(10);

    for file_count in [10_usize, 100_usize, 500_usize] {
        group.throughput(Throughput::Elements(file_count as u64));
        group.bench_function(format!("full_refresh_{file_count}_files"), |b| {
            b.iter(|| {
                let client_dir = temp_client_dir(&format!("full-refresh-{file_count}"));
                let _ = std::fs::remove_dir_all(&client_dir);
                std::fs::create_dir_all(&client_dir).expect("create source-index bench client dir");
                let report = ClientDbEngine::refresh_source_index_import_from_client_dir(
                    &client_dir,
                    refresh_request(&project_root, "source-index-full-refresh", file_count),
                )
                .expect("refresh source-index benchmark import");
                assert_eq!(report.file_count, file_count.min(u32::MAX as usize) as u32);
                assert!(!report.reused_generation);
                std::hint::black_box(report);
                let _ = std::fs::remove_dir_all(&client_dir);
            });
        });
    }

    let reuse_dir = temp_client_dir("reuse");
    let _ = std::fs::remove_dir_all(&reuse_dir);
    std::fs::create_dir_all(&reuse_dir).expect("create source-index reuse bench client dir");
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &reuse_dir,
        refresh_request(&project_root, "source-index-reuse", 100),
    )
    .expect("seed reusable source-index generation");
    group.throughput(Throughput::Elements(100));
    group.bench_function("reuse_refresh_100_files", |b| {
        b.iter(|| {
            let report = ClientDbEngine::refresh_source_index_import_from_client_dir(
                std::hint::black_box(&reuse_dir),
                refresh_request(&project_root, "source-index-reuse", 100),
            )
            .expect("reuse source-index benchmark import");
            assert!(report.reused_generation);
            std::hint::black_box(report);
        });
    });
    let _ = std::fs::remove_dir_all(&reuse_dir);
    group.finish();
}

criterion_group!(benches, turso_source_index);
criterion_main!(benches);
