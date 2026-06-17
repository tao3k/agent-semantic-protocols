use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheManifest, LanguageId,
    ProviderId,
};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationLookup};
use serde_json::json;

#[test]
fn invalidate_generations_for_project_preserves_other_project_rows() {
    let root = temp_root("invalidate-project");
    let other_root = temp_root("invalidate-project-other");
    let db_path = root.join("client.sqlite3");
    let main_manifest = manifest(&root);
    let mut other_manifest = manifest(&other_root);
    other_manifest.generations[0].generation_id = CacheGenerationId::from("rust-other-1");
    other_manifest.generations[0].artifact_ids =
        Some(vec![CacheArtifactId::from("search/rust-other-1.json")]);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&main_manifest).expect("import manifest");
    db.import_manifest(&other_manifest)
        .expect("import other manifest");
    let invalidated = ClientDb::invalidate_generations_for_project(&db_path, &root)
        .expect("invalidate project generations");
    let has_generation = has_prime_generation(&db_path, &root);
    let has_other_generation = has_prime_generation(&db_path, &other_root);

    assert_eq!(invalidated, 1);
    assert!(!has_generation);
    assert!(has_other_generation);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(other_root);
}

fn has_prime_generation(db_path: &std::path::Path, root: &std::path::Path) -> bool {
    ClientDb::has_generation(&ClientDbGenerationLookup {
        db_path: db_path.to_path_buf(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        export_method: CacheExportMethod::from("search/prime"),
        request_fingerprint: None,
    })
    .expect("generation lookup after invalidate")
}

fn manifest(root: &std::path::Path) -> ClientCacheManifest {
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
                "rawSourceStored": false,
                "requestFingerprint": "fnv64:0123456789abcdef",
                "fileHashes": [{
                    "path": "src/lib.rs",
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "byteLen": 1,
                    "mtimeMs": 1
                }],
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
