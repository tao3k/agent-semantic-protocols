use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexLookupState,
};

#[tokio::test(flavor = "current_thread")]
async fn source_index_lookup_reports_cold_required_for_precanonical_storage() {
    let temp_root = temp_project_root("source-index-legacy-selector-payload");
    let client_dir = temp_root.join("client");
    let project_root = temp_root.join("workspace");
    fs::create_dir_all(&client_dir).expect("create client dir");
    fs::create_dir_all(&project_root).expect("create project dir");
    create_legacy_source_index_db(&client_dir, &project_root).await;

    let result = ClientDbEngine::lookup_source_index_from_client_dir(
        ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &client_dir,
            indexed_project_root: &project_root,
            language_id: None,
            query_keys: vec!["legacy_symbol".into()],
            limit: 8,
        },
    )
    .expect("lookup reports legacy source-index selector schema");

    assert_eq!(result.state, ClientDbSourceIndexLookupState::ColdRequired);
    assert!(result.candidates.is_empty());

    let _ = fs::remove_dir_all(temp_root);
}

async fn create_legacy_source_index_db(client_dir: &Path, project_root: &Path) {
    let db_path = client_dir.join("client.turso");
    let database = turso::Builder::new_local(db_path.to_string_lossy().as_ref())
        .build()
        .await
        .expect("open legacy source-index db");
    let connection = database.connect().expect("connect legacy source-index db");
    for statement in [
        "CREATE TABLE asp_source_index_generation (
            generation_id TEXT PRIMARY KEY,
            project_root TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            file_hashes_json TEXT NOT NULL,
            owner_count INTEGER NOT NULL,
            selector_count INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        )",
        "CREATE TABLE asp_source_index_owner (
            generation_id TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            language_id TEXT,
            provider_id TEXT,
            source_kind TEXT NOT NULL,
            line_count INTEGER,
            query_keys_json TEXT NOT NULL,
            PRIMARY KEY (generation_id, owner_path)
        )",
        "CREATE TABLE asp_source_index_selector (
            generation_id TEXT NOT NULL,
            owner_path TEXT NOT NULL,
            selector_id TEXT NOT NULL,
            symbol TEXT,
            kind TEXT,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            source TEXT NOT NULL,
            query_keys_json TEXT NOT NULL,
            PRIMARY KEY (generation_id, selector_id)
        )",
    ] {
        connection
            .execute(statement, ())
            .await
            .expect("create legacy source-index schema");
    }
    connection
        .execute(
            "INSERT INTO asp_source_index_generation
             (generation_id, project_root, schema_id, schema_version, file_hashes_json, owner_count, selector_count, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                "gen-legacy",
                project_root.to_string_lossy().as_ref(),
                "agent.semantic-protocols.semantic-source-index",
                "1",
                "[]",
                1_i64,
                1_i64,
                1_i64,
            ),
        )
        .await
        .expect("insert legacy generation");
    connection
        .execute(
            "INSERT INTO asp_source_index_owner
             (generation_id, owner_path, language_id, provider_id, source_kind, line_count, query_keys_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                "gen-legacy",
                "src/legacy.rs",
                "rust",
                "rs-harness",
                "provider",
                12_i64,
                "[\"legacy_symbol\"]",
            ),
        )
        .await
        .expect("insert legacy owner");
    connection
        .execute(
            "INSERT INTO asp_source_index_selector
             (generation_id, owner_path, selector_id, symbol, kind, start_line, end_line, source, query_keys_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                "gen-legacy",
                "src/legacy.rs",
                "rust://src/legacy.rs#item/function/legacy_symbol",
                "legacy_symbol",
                "function",
                1_i64,
                3_i64,
                "fn legacy_symbol() {}",
                "[\"legacy_symbol\"]",
            ),
        )
        .await
        .expect("insert legacy selector");
}

fn temp_project_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-{label}-{nonce}"))
}
