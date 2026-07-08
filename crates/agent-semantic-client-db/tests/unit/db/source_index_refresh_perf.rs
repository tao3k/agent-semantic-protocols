use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{ClientCacheFileHash, LanguageId, ProviderId};
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexImport, ClientDbSourceIndexOwner,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSelector,
    client_db_source_index_generation_id,
};

const SOURCE_INDEX_WARM_REUSE_GATE: Duration = Duration::from_millis(750);
const SOURCE_INDEX_HASH_REUSE_GATE: Duration = Duration::from_millis(25);

#[tokio::test(flavor = "current_thread")]
async fn source_index_refresh_reuse_stays_on_structured_turso_path() {
    let root = temp_project_root("source-index-refresh-reuse-perf");
    let client_dir = root.join("client");
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let request = refresh_request(&project_root);
    let first =
        ClientDbEngine::refresh_source_index_import_from_client_dir(&client_dir, request.clone())
            .expect("cold source-index refresh");
    assert!(!first.reused_generation, "first refresh should write rows");

    let started_at = Instant::now();
    let second = ClientDbEngine::refresh_source_index_import_from_client_dir(&client_dir, request)
        .expect("warm source-index refresh");
    let elapsed = started_at.elapsed();

    assert!(second.reused_generation, "second refresh should reuse rows");
    assert_eq!(second.owner_count, 1);
    assert_eq!(second.selector_count, 1);
    assert!(
        elapsed < SOURCE_INDEX_WARM_REUSE_GATE,
        "warm source-index refresh should stay on structured Turso path; elapsed={elapsed:?} gate={SOURCE_INDEX_WARM_REUSE_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_hash_reuse_ignores_scope_dir_mtime() {
    let root = temp_project_root("source-index-hash-reuse-dir-mtime");
    let project_root = root.join("project");
    let source_dir = project_root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create project src dir");
    let source_path = source_dir.join("source_index_perf.rs");
    std::fs::write(&source_path, "pub fn source_index_perf_fixture() {}\n")
        .expect("write source fixture");
    let files = vec![agent_semantic_client_db::ClientDbSourceIndexScopeFile {
        path: source_path,
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        selector_receipts: Vec::new(),
    }];

    let first = agent_semantic_client_db::source_index_file_hashes(
        &project_root,
        &files,
        None,
        "registry-fingerprint",
        std::iter::empty(),
    )
    .expect("initial source-index file hashes");

    let transient = source_dir.join(".transient-source-index-mtime");
    std::fs::write(&transient, "mtime witness").expect("write transient file");
    std::fs::remove_file(&transient).expect("remove transient file");

    let started_at = Instant::now();
    let second = agent_semantic_client_db::source_index_file_hashes(
        &project_root,
        &files,
        Some(&first),
        "registry-fingerprint",
        std::iter::empty(),
    )
    .expect("reused source-index file hashes");
    let elapsed = started_at.elapsed();

    assert_eq!(
        second, first,
        "source-index no-op fingerprint must ignore directory mtime churn"
    );
    assert!(
        elapsed < SOURCE_INDEX_HASH_REUSE_GATE,
        "source-index hash reuse should remain millisecond-scale; elapsed={elapsed:?} gate={SOURCE_INDEX_HASH_REUSE_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

fn refresh_request(project_root: &Path) -> ClientDbSourceIndexRefreshRequest {
    ClientDbSourceIndexRefreshRequest {
        file_count: 1,
        import: ClientDbSourceIndexImport {
            generation_id: client_db_source_index_generation_id(),
            project_root: project_root.to_path_buf(),
            schema_id: "agent-semantic-client-db.source-index".to_string().into(),
            schema_version: "1".into(),
            file_hashes: vec![ClientCacheFileHash {
                path: "src/source_index_perf.rs".to_string(),
                sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                byte_len: 42,
                mtime_ms: 1,
            }],
            owners: vec![ClientDbSourceIndexOwner {
                owner_path: "src/source_index_perf.rs".into(),
                language_id: Some(LanguageId::from("rust")),
                provider_id: Some(ProviderId::from("rs-harness")),
                source_kind: "source".to_string().into(),
                line_count: Some(8),
                query_keys: vec!["source_index_perf_fixture".to_string().into()],
            }],
            selectors: vec![ClientDbSourceIndexSelector {
                owner_path: "src/source_index_perf.rs".into(),
                selector_id: "source_index_perf_fixture".to_string(),
                symbol: Some("source_index_perf_fixture".to_string()),
                kind: Some("function".to_string()),
                start_line: 1,
                end_line: 3,
                source: "pub fn source_index_perf_fixture() {}".to_string().into(),
                payload_proof: None,
                query_keys: vec!["source_index_perf_fixture".to_string().into()],
            }],
        },
    }
}

fn temp_project_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-{label}-{nonce}"))
}
