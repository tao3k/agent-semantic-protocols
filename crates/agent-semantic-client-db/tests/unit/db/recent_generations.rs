use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{CacheExportMethod, ClientCacheManifest, LanguageId, ProviderId};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationLookup};
use serde_json::json;

#[test]
fn lookup_recent_generations_returns_newest_matching_candidates() {
    let root = temp_root("recent-generations");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest_with_generation(
        &root,
        "older-fzf",
        "search/fzf",
        "search/older-fzf.json",
    ))
    .expect("import older manifest");
    std::thread::sleep(Duration::from_secs(1));
    db.import_manifest(&manifest_with_generation(
        &root,
        "newer-fzf",
        "search/fzf",
        "search/newer-fzf.json",
    ))
    .expect("import newer manifest");

    let lookup = ClientDbGenerationLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        export_method: CacheExportMethod::from("search/fzf"),
        request_fingerprint: None,
    };
    let hits = ClientDb::lookup_recent_generations(&lookup, 10).expect("lookup recent");
    let limited = ClientDb::lookup_recent_generations(&lookup, 1).expect("lookup limited");
    let exact = ClientDb::lookup_recent_generations(
        &ClientDbGenerationLookup {
            request_fingerprint: Some("fnv64:older-fzf".to_string()),
            ..lookup
        },
        10,
    )
    .expect("lookup exact");

    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].artifact_ids[0].as_str(), "search/newer-fzf.json");
    assert_eq!(hits[1].artifact_ids[0].as_str(), "search/older-fzf.json");
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].artifact_ids[0].as_str(), "search/newer-fzf.json");
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].artifact_ids[0].as_str(), "search/older-fzf.json");
    let _ = std::fs::remove_dir_all(root);
}

fn manifest_with_generation(
    root: &std::path::Path,
    generation_id: &str,
    export_method: &str,
    artifact_id: &str,
) -> ClientCacheManifest {
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": root.display().to_string(),
        "generations": [
            {
                "generationId": generation_id,
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": export_method,
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": format!("fnv64:{generation_id}"),
                "fileHashes": [{"path": "src/lib.rs", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"}],
                "artifactIds": [artifact_id]
            }
        ]
    }))
    .expect("manifest with generation")
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
