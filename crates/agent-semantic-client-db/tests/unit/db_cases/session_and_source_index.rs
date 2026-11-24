// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CacheGenerationId, ClientCacheFileHash,
    ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexSource, LanguageId, ProviderId,
    SemanticSchemaId, SemanticSchemaVersion, build_fixture_source_index_import,
    client_db_source_index_file_count, source_index_relative_path, source_index_scope_dirs,
    temp_root,
};

#[tokio::test]
async fn source_index_import_assembly_uses_turso_ready_contract_rows() {
    let root = temp_root("source-index-import");
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create src dir");
    let lib = src.join("lib.rs");
    let source = b"pub fn turso_source_index_fixture() {}\n";
    std::fs::write(&lib, source).expect("write source");
    let selector = "rust://src/lib.rs#item/function/turso_source_index_fixture";
    let scope_file = ClientDbSourceIndexScopeFile {
        relations: Vec::new(),
        path: lib.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("asp-rust"),
        projection_coverage:
            agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::Complete,
        projection_diagnostic: None,
        selector_receipts: vec![agent_semantic_client_db::ClientDbSourceIndexSelector {
            owner_path: "src/lib.rs".into(),
            provider_id: ProviderId::from("asp-rust"),
            selector_id: selector.into(),
            symbol: Some("turso_source_index_fixture".into()),
            kind: Some("function".into()),
            source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            query_keys: vec!["turso_source_index_fixture".into()],
            derived_projections: Vec::new(),
            projection_record: crate::projection_fixture::projection_record(
                crate::projection_fixture::ProjectionFixtureInput {
                    language_id: "rust",
                    provider_id: "asp-rust",
                    owner_path: "src/lib.rs",
                    structural_selector: selector,
                    item_kind: "function",
                    item_name: "turso_source_index_fixture",
                    source,
                    source_byte_start: 0,
                    source_byte_end: source.len() as u64,
                },
            ),
        }],
    };

    let import = agent_semantic_client_db::assemble_source_index_import(
        ClientDbSourceIndexImportAssemblyRequest {
            generation_id: CacheGenerationId::from("source-index-generation"),
            project_root: root.clone(),
            schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
            selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            file_text_bytes_limit: 4096,
            registry_fingerprint: "registry:v1".to_string(),
            extra_scope_dirs: Vec::new(),
            files: vec![scope_file.clone()],
            source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
                [(
                    agent_semantic_client_db::ClientDbSourceIndexPath::new("src/lib.rs"),
                    b"pub fn turso_source_index_fixture() {}\n".to_vec(),
                )],
            ),
        },
    )
    .expect("assemble source-index import");

    assert_eq!(source_index_relative_path(&root, &lib), "src/lib.rs");
    assert_eq!(
        source_index_scope_dirs(&root, &[scope_file]),
        [".", "src"].into_iter().map(str::to_string).collect()
    );
    assert_eq!(client_db_source_index_file_count(usize::MAX), u32::MAX);
    assert_eq!(import.owners.len(), 1);
    assert_eq!(import.selectors.len(), 1);
    assert!(
        import
            .file_hashes
            .iter()
            .any(|hash| hash.path == "src/lib.rs")
    );
    assert!(
        import
            .owners
            .first()
            .expect("owner")
            .query_keys
            .iter()
            .any(|key| key.as_str() == "turso_source_index_fixture")
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn source_index_refresh_request_remains_db_engine_owned() {
    let root = temp_root("source-index-refresh");
    let import = build_fixture_source_index_import(ClientDbSourceIndexImportRequest {
        source_blobs: Default::default(),
        generation_id: CacheGenerationId::from("source-index-refresh-generation"),
        project_root: root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "0123456789abcdef".repeat(4),
            byte_len: 37,
            mtime_ms: 42,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relations: Vec::new(),
            relative_path: "src/lib.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("asp-rust"),
            text: "pub fn source_index_refresh_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build source-index import");

    let request = ClientDbSourceIndexRefreshRequest {
        file_count: 1,
        import,
        source_snapshot: crate::snapshot_fixture::source_snapshot_evidence(),
    };
    assert_eq!(request.file_count, 1);
    assert_eq!(
        request.import.schema_id.as_str(),
        CLIENT_DB_SOURCE_INDEX_SCHEMA_ID
    );

    let _ = std::fs::remove_dir_all(root);
}
