use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{CacheExportMethod, ClientCacheManifest, LanguageId, ProviderId};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationLookup, ClientDbStatus};
use serde_json::json;

#[test]
fn schema_version_stays_on_first_release_contract() {
    assert_eq!(
        agent_semantic_client_db::AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION,
        1
    );
}

#[test]
fn inspect_reports_missing_without_creating_db() {
    let root = temp_root("missing");
    let db_path = root.join("client.sqlite3");

    let report = ClientDb::inspect(&db_path);

    assert_eq!(report.status, ClientDbStatus::Missing);
    assert!(
        ClientDb::open_read_only_existing(&db_path)
            .expect("open missing read-only db")
            .is_none()
    );
    assert_eq!(report.generation_count, 0);
    assert_eq!(report.syntax_row_generation_count, 0);
    assert_eq!(report.syntax_row_match_count, 0);
    assert_eq!(report.syntax_row_capture_count, 0);
    assert!(!db_path.exists());
    assert!(report.runtime_pragmas.is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn open_creates_schema_and_imports_manifest_generations() {
    let root = temp_root("import");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root, false);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let summary = db.summary().expect("db summary");
    let generation_hit = ClientDb::lookup_generation(&ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("search/prime"),
        request_fingerprint: Some("fnv64:0123456789abcdef".to_string()),
    })
    .expect("lookup generation")
    .expect("generation hit");
    let report = ClientDb::inspect(&db_path);
    let read_db = ClientDb::open_read_only_existing(&db_path)
        .expect("open read-only db")
        .expect("db exists");
    let open_report = read_db.inspect_open().expect("inspect open db");
    let stored_schema_version: String = rusqlite::Connection::open(&db_path)
        .expect("open sqlite")
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schemaVersion'",
            [],
            |row| row.get(0),
        )
        .expect("schema version");
    let journal_mode: String = rusqlite::Connection::open(&db_path)
        .expect("open sqlite")
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal mode");

    assert_eq!(summary.generation_count, 1);
    assert_eq!(summary.syntax_row_generation_count, 0);
    assert_eq!(summary.syntax_row_match_count, 0);
    assert_eq!(summary.syntax_row_capture_count, 0);
    assert!(!summary.raw_source_stored);
    assert_eq!(report.status, ClientDbStatus::Present);
    assert_eq!(report.generation_count, 1);
    assert_eq!(report.syntax_row_generation_count, 0);
    assert_eq!(report.syntax_row_match_count, 0);
    assert_eq!(report.syntax_row_capture_count, 0);
    assert!(!report.raw_source_stored);
    assert_eq!(open_report, report);
    let runtime_pragmas = report.runtime_pragmas.expect("runtime pragmas");
    assert_eq!(runtime_pragmas.journal_mode.as_str(), "wal");
    assert_eq!(runtime_pragmas.busy_timeout_ms, 5000);
    assert!(runtime_pragmas.foreign_keys);
    assert_eq!(
        stored_schema_version,
        agent_semantic_client_db::AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION.to_string()
    );
    assert_eq!(generation_hit.file_hashes.len(), 1);
    assert_eq!(generation_hit.file_hashes[0].path, "src/lib.rs");
    assert_eq!(generation_hit.file_hashes[0].sha256, "0".repeat(64));
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn open_waits_for_transient_writer_lock() {
    let root = temp_root("writer-lock");
    let db_path = root.join("client.sqlite3");
    let db = ClientDb::open_or_create(&db_path).expect("create db");
    drop(db);

    let (ready_tx, ready_rx) = mpsc::channel();
    let locked_db_path = db_path.clone();
    let lock_handle = thread::spawn(move || {
        let conn = rusqlite::Connection::open(&locked_db_path).expect("open lock connection");
        conn.execute_batch("BEGIN IMMEDIATE")
            .expect("begin write lock");
        ready_tx.send(()).expect("send lock ready");
        thread::sleep(Duration::from_millis(200));
        conn.execute_batch("COMMIT").expect("release write lock");
    });

    ready_rx.recv().expect("wait for write lock");
    let reopened = ClientDb::open_or_create(&db_path).expect("open waits for writer lock");
    drop(reopened);
    lock_handle.join().expect("lock thread");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn has_generation_matches_language_provider_project_and_export_method() {
    let root = temp_root("has-generation");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root, false);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");

    assert!(
        ClientDb::has_generation(&ClientDbGenerationLookup {
            db_path: db_path.clone(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            project_root: root.clone(),
            export_method: CacheExportMethod::from("search/prime"),
            request_fingerprint: None,
        })
        .expect("has generation")
    );
    let hit = ClientDb::lookup_generation(&ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("search/prime"),
        request_fingerprint: None,
    })
    .expect("lookup generation")
    .expect("generation hit");
    assert_eq!(
        hit.request_fingerprint.as_deref(),
        Some("fnv64:0123456789abcdef")
    );
    assert_eq!(hit.schema_ids.len(), 1);
    assert_eq!(
        hit.schema_ids[0].as_str(),
        "agent.semantic-protocols.semantic-search-packet"
    );
    assert_eq!(hit.artifact_ids.len(), 1);
    assert_eq!(hit.artifact_ids[0].as_str(), "search/rust-main-1.json");
    assert!(
        !ClientDb::has_generation(&ClientDbGenerationLookup {
            db_path: db_path.clone(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            project_root: root.clone(),
            export_method: CacheExportMethod::from("search/owner"),
            request_fingerprint: None,
        })
        .expect("missing generation")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalidate_generations_clears_rows_without_deleting_db() {
    let root = temp_root("invalidate");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root, false);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let invalidated = ClientDb::invalidate_generations(&db_path).expect("invalidate generations");
    let report = ClientDb::inspect(&db_path);
    let has_generation = ClientDb::has_generation(&ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("search/prime"),
        request_fingerprint: None,
    })
    .expect("generation lookup after invalidate");

    assert_eq!(invalidated, 1);
    assert_eq!(report.status, ClientDbStatus::Present);
    assert_eq!(report.generation_count, 0);
    assert!(!has_generation);
    assert!(db_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalidate_generations_is_noop_when_db_is_missing() {
    let root = temp_root("invalidate-missing");
    let db_path = root.join("client.sqlite3");

    let invalidated = ClientDb::invalidate_generations(&db_path).expect("invalidate missing db");

    assert_eq!(invalidated, 0);
    assert!(!db_path.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn import_rejects_raw_source_generations() {
    let root = temp_root("raw-source");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root, true);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    let error = db
        .import_manifest(&manifest)
        .expect_err("reject raw source");

    assert!(error.contains("rawSourceStored=true"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

fn manifest(root: &std::path::Path, raw_source_stored: bool) -> ClientCacheManifest {
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": root.display().to_string(),
        "generations": [
            {
                "generationId": "rust-main-1",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "search/prime",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "miss",
                "rawSourceStored": raw_source_stored,
                "requestFingerprint": "fnv64:0123456789abcdef",
                "fileHashes": [{"path": "src/lib.rs", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"}],
                "artifactIds": ["search/rust-main-1.json"]
            }
        ]
    }))
    .expect("manifest")
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-db-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
