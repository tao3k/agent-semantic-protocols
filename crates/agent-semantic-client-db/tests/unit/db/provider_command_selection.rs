use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::{ClientDb, ClientDbProviderCommandSelection};

#[test]
fn provider_command_selection_cache_round_trips_context_rows() {
    let root = temp_root("provider-command-selection");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let row = ClientDbProviderCommandSelection::new(
        "agent.semantic-protocols.languages.rust.rs-harness".to_string(),
        "sha256:abc".to_string(),
        "rust".to_string(),
        "rs-harness".to_string(),
        "rs-harness".to_string(),
        "external-process".to_string(),
        vec!["/tmp/rs-harness".to_string()],
        Some("/tmp/rs-harness".to_string()),
        Some(42),
        Some(1234),
    );

    db.replace_provider_command_selections(&root, "sha256:ctx-a", std::slice::from_ref(&row))
        .expect("write provider selection");
    let hit = db
        .lookup_provider_command_selections(&root, "sha256:ctx-a")
        .expect("lookup provider selection")
        .expect("provider selection rows");
    let miss = db
        .lookup_provider_command_selections(&root, "sha256:ctx-b")
        .expect("lookup miss");

    assert_eq!(hit, vec![row]);
    assert!(miss.is_none());
    let _ = std::fs::remove_dir_all(root);
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
