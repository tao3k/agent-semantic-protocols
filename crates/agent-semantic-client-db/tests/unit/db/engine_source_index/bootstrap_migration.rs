use std::fs;

use super::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CacheGenerationId, ClientCacheFileHash, ClientDbEngine,
    ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexImportFile,
    ClientDbSourceIndexImportRequest, ClientDbSourceIndexLookupState, ClientDbSourceIndexQueryKey,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSource, LanguageId, ProviderId,
    SemanticSchemaId, SemanticSchemaVersion, build_source_index_import, temp_root,
};

#[tokio::test(flavor = "current_thread")]
async fn db_engine_source_index_bootstrap_converges_historical_owner_schema_columns() {
    let client_dir = temp_root("db-engine-source-index-migration-client");
    let project_root = temp_root("db-engine-source-index-migration-project");
    fs::create_dir_all(&client_dir).expect("create client dir");
    let db_path = client_dir.join("client.turso");
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("create historical source-index fixture database");
        let connection = database
            .connect()
            .expect("connect historical source-index fixture database");
        connection
            .execute(
                "CREATE TABLE asp_source_index_owner (
                    generation_id TEXT NOT NULL,
                    owner_path TEXT NOT NULL,
                    PRIMARY KEY (generation_id, owner_path)
                )",
                (),
            )
            .await
            .expect("create historical source-index owner schema");
    }

    let source_index_import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from("source-index-migrated-turso"),
        project_root: project_root.clone(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: vec![ClientCacheFileHash {
            path: "src/source_index_migrated_turso.rs".to_string(),
            sha256: "abcdef1234567890".repeat(4),
            byte_len: 51,
            mtime_ms: 12,
        }],
        files: vec![ClientDbSourceIndexImportFile {
            relative_path: "src/source_index_migrated_turso.rs".to_string(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            text: "pub fn source_index_migrated_turso_fixture() {}\n".to_string(),
            selectors: Vec::new(),
        }],
    })
    .expect("build migrated Turso source-index import");

    let refresh = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import.clone(),
            file_count: 1,
        },
    )
    .expect("refresh source-index through migrated Turso owner schema");
    assert_eq!(
        refresh.generation_id.as_str(),
        "source-index-migrated-turso"
    );
    assert_eq!(refresh.owner_count, 1);
    assert_eq!(refresh.selector_count, 1);

    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("open canonical source-index fixture database");
        let connection = database
            .connect()
            .expect("connect canonical source-index fixture database");
        connection
            .execute("DROP TABLE asp_source_index_token_owner_v1", ())
            .await
            .expect("remove token-owner table from historical fixture");
    }

    let restored_refresh = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: source_index_import,
            file_count: 1,
        },
    )
    .expect("refresh source-index after token-owner migration");
    assert_ne!(
        restored_refresh.generation_id.as_str(),
        "source-index-migrated-turso"
    );
    assert_ne!(restored_refresh.generation_id, refresh.generation_id);
    {
        let db_path_string = db_path.display().to_string();
        let database = turso::Builder::new_local(&db_path_string)
            .experimental_index_method(true)
            .build()
            .await
            .expect("reopen migrated source-index fixture database");
        let connection = database
            .connect()
            .expect("reconnect migrated source-index fixture database");
        connection
            .query("SELECT 1 FROM asp_source_index_token_owner_v1 LIMIT 1", ())
            .await
            .expect("token-owner table restored by source-index migration");
    }

    let rust_language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        "source_index_migrated_turso_fixture",
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("lookup migrated Turso source-index read model");
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        lookup.candidates.iter().any(|candidate| candidate.path
            == "src/source_index_migrated_turso.rs"
            && candidate.language_id.as_ref().map(|id| id.as_str()) == Some("rust")
            && candidate.provider_id.as_ref().map(|id| id.as_str()) == Some("rs-harness")),
        "lookup={lookup:?}"
    );
    let facade_lookup = ClientDbEngine::lookup_source_index_from_client_dir(
        ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &client_dir,
            indexed_project_root: &project_root,
            language_id: Some(&rust_language_id),
            query_keys: vec![ClientDbSourceIndexQueryKey::from(
                "source_index_migrated_turso_fixture",
            )],
            limit: 8,
        },
    )
    .expect("lookup source-index through facade with migrated Turso owner schema");
    assert_eq!(facade_lookup.state, ClientDbSourceIndexLookupState::Hit);
    assert!(
        facade_lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/source_index_migrated_turso.rs"),
        "facade_lookup={facade_lookup:?}"
    );

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}
