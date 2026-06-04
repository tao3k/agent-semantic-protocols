use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{CacheExportMethod, ClientCacheManifest, LanguageId, ProviderId};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationLookup, ClientDbStatus};
use serde_json::json;

#[test]
fn inspect_reports_missing_without_creating_db() {
    let root = temp_root("missing");
    let db_path = root.join("client.sqlite3");

    let report = ClientDb::inspect(&db_path);

    assert_eq!(report.status, ClientDbStatus::Missing);
    assert_eq!(report.generation_count, 0);
    assert!(!db_path.exists());
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
    let report = ClientDb::inspect(&db_path);

    assert_eq!(summary.generation_count, 1);
    assert!(!summary.raw_source_stored);
    assert_eq!(report.status, ClientDbStatus::Present);
    assert_eq!(report.generation_count, 1);
    assert!(!report.raw_source_stored);
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
        })
        .expect("has generation")
    );
    let hit = ClientDb::lookup_generation(&ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("search/prime"),
    })
    .expect("lookup generation")
    .expect("generation hit");
    assert_eq!(hit.artifact_ids.len(), 1);
    assert_eq!(hit.artifact_ids[0].as_str(), "search/rust-main-1.json");
    assert!(
        !ClientDb::has_generation(&ClientDbGenerationLookup {
            db_path: db_path.clone(),
            language_id: LanguageId::from("rust"),
            provider_id: ProviderId::from("rs-harness"),
            project_root: root.clone(),
            export_method: CacheExportMethod::from("search/owner"),
        })
        .expect("missing generation")
    );
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
                "fileHashes": [],
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
