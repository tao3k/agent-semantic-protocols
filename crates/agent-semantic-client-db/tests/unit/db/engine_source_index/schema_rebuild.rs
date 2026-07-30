use super::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CacheGenerationId, ClientCacheFileHash, ClientDbEngine,
    ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSource,
    LanguageId, ProviderId, SemanticSchemaId, SemanticSchemaVersion, build_source_index_import,
    temp_root,
};
use std::fs;

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_refresh_rebuilds_noncanonical_snapshot_schema() {
    let client_dir = temp_root("db-engine-source-index-canonical-schema-client");
    let project_root = temp_root("db-engine-source-index-canonical-schema-project");
    let source_snapshot = crate::snapshot_fixture::source_snapshot_evidence();
    fs::create_dir_all(&client_dir).expect("create client dir");
    let db_path = client_dir.join("facts.turso");
    ClientDbEngine::open_write_session_client_dir(&client_dir).expect("stage canonical Turso DB");
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("create noncanonical source-index database");
        let connection = database
            .connect()
            .expect("connect noncanonical source-index database");
        connection
            .execute("DROP TABLE asp_source_index_scope_v1", ())
            .await
            .expect("drop canonical source-index scope table");
        connection
            .execute(
                "CREATE TABLE asp_source_index_scope_v1 (obsolete_marker TEXT NOT NULL)",
                (),
            )
            .await
            .expect("create noncanonical source-index scope table");
    }
    let cold_lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &source_snapshot,
        "canonical_schema",
        Some(&LanguageId::from("rust")),
        1,
    )
    .await
    .expect("read-only lookup of noncanonical source-index namespace");
    assert_eq!(
        cold_lookup.state,
        ClientDbSourceIndexLookupState::ColdRequired
    );
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("reopen noncanonical source-index database after lookup");
        let connection = database
            .connect()
            .expect("connect noncanonical source-index database after lookup");
        let mut rows = connection
            .query("PRAGMA table_info(asp_source_index_scope_v1)", ())
            .await
            .expect("inspect noncanonical schema after read-only lookup");
        let mut saw_obsolete_marker = false;
        while let Some(row) = rows.next().await.expect("read noncanonical scope column") {
            saw_obsolete_marker |= row
                .get::<String>(1)
                .expect("read noncanonical source-index column")
                == "obsolete_marker";
        }
        assert!(
            saw_obsolete_marker,
            "read-only lookup must not mutate a noncanonical derived namespace"
        );
    }
    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-canonical-schema-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/canonical_schema.rs".to_string(),
            sha256: "5555555555555555".repeat(4),
            byte_len: 1,
            mtime_ms: 55,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/canonical_schema.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "fn canonical_schema() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build canonical source-index import");
    let source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized([(
            agent_semantic_client_db::ClientDbSourceIndexPath::new("src/canonical_schema.rs"),
            b"fn canonical_schema() {}\n".to_vec(),
        )]);
    let refresh =
        agent_semantic_client_db::fixture::commit_source_index_generation_from_fixture_dir(
            &client_dir,
            ClientDbSourceIndexRefreshRequest {
                import: source_index_import,
                file_count: 1,
                source_snapshot: source_snapshot.clone(),
            },
            &source_blobs,
        )
        .expect("bootstrap canonical source-index schema");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &refresh.source_snapshot,
        "canonical_schema",
        Some(&LanguageId::from("rust")),
        1,
    )
    .await
    .expect("lookup after canonical schema bootstrap");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("reopen canonical source-index database");
        let connection = database
            .connect()
            .expect("connect canonical source-index database");
        let mut rows = connection
            .query("PRAGMA table_info(asp_source_index_scope_v1)", ())
            .await
            .expect("inspect canonical source-index scope columns");
        let mut columns = Vec::new();
        while let Some(row) = rows.next().await.expect("read source-index scope column") {
            columns.push(row.get::<String>(1).expect("read source-index column name"));
        }
        assert!(
            columns
                .iter()
                .any(|column| column == "source_snapshot_json")
        );
        assert!(
            columns
                .iter()
                .any(|column| column == "selector_fingerprint")
        );
        assert!(!columns.iter().any(|column| column == "obsolete_marker"));
    }
    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}
