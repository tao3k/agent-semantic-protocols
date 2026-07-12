use std::fs;

use super::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CacheGenerationId, ClientCacheFileHash, ClientDbEngine,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSource,
    LanguageId, ProviderId, SemanticSchemaId, SemanticSchemaVersion, build_source_index_import,
    temp_root,
};

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_lookup_reads_canonical_snapshot() {
    let client_dir = temp_root("db-engine-source-index-canonical-snapshot-client");
    let project_root = temp_root("db-engine-source-index-canonical-snapshot-project");
    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-canonical-snapshot-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/canonical_snapshot.rs".to_string(),
            sha256: "4444444444444444".repeat(4),
            byte_len: 1,
            mtime_ms: 44,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/canonical_snapshot.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn canonical_snapshot_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build canonical source-index import");
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import,
            file_count: 1,
        },
    )
    .expect("write canonical source-index snapshot");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "canonical_snapshot_fixture",
        Some(&LanguageId::from("rust")),
        8,
    )
    .await
    .expect("lookup canonical source-index snapshot");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/canonical_snapshot.rs")
    );
    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}
#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_lookup_request_stays_within_project_scope() {
    let client_dir = temp_root("db-engine-source-index-scope-binding-client");
    let project_a = temp_root("db-engine-source-index-scope-binding-project-a");
    let project_b = temp_root("db-engine-source-index-scope-binding-project-b");
    let import_a = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-scope-a"),
        project_root: project_a.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/scope_a.rs".to_string(),
            sha256: "aaaaaaaaaaaaaaaa".repeat(4),
            byte_len: 1,
            mtime_ms: 1,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/scope_a.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn scope_a_symbol() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build project A source-index import");
    let import_b = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-scope-b"),
        project_root: project_b.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/scope_b.rs".to_string(),
            sha256: "bbbbbbbbbbbbbbbb".repeat(4),
            byte_len: 1,
            mtime_ms: 2,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/scope_b.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn scope_b_symbol() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build project B source-index import");
    for import in [import_a, import_b] {
        ClientDbEngine::refresh_source_index_import_from_client_dir(
            &client_dir,
            ClientDbSourceIndexRefreshRequest {
                import,
                file_count: 1,
            },
        )
        .expect("write scoped source-index snapshot");
    }

    let language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_from_client_dir(
        agent_semantic_client_db::ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &client_dir,
            indexed_project_root: &project_a,
            language_id: Some(&language_id),
            query_keys: vec!["scope_a_symbol".into()],
            limit: 8,
        },
    )
    .expect("lookup project A scope");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        lookup
            .candidates
            .iter()
            .all(|candidate| candidate.path == "src/scope_a.rs")
    );
    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_a);
    let _ = fs::remove_dir_all(project_b);
}
