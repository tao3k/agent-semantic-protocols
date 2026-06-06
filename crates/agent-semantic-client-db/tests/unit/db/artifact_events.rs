use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::{ClientDb, ClientDbArtifactEvent};

#[test]
fn artifact_event_index_upserts_and_reads_timeline_rows() {
    let root = temp_root("artifact-events");
    let db_path = root.join("client.sqlite3");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    let written = db
        .upsert_artifact_events(&[
            event("prompt-output/rust.command.json", 0, 1000, "search/owner"),
            event("prompt-output/rust.command.json", 1, 1001, "query"),
            event("search/rust-search-owner.json", 0, 1100, "search/owner"),
        ])
        .expect("write artifact events");
    let rewritten = db
        .upsert_artifact_events(&[event("prompt-output/rust.command.json", 1, 1200, "query")])
        .expect("rewrite artifact event");

    let summary = db.summary().expect("db summary");
    let inspected = ClientDb::inspect(&db_path);
    let all = ClientDb::lookup_artifact_events(&db_path, None, 10).expect("read events");
    let recent =
        ClientDb::lookup_artifact_events(&db_path, Some(1100), 10).expect("read recent events");

    assert_eq!(written, 3);
    assert_eq!(rewritten, 1);
    assert_eq!(summary.artifact_event_count, 3);
    assert_eq!(inspected.artifact_event_count, 3);
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].artifact_path, "prompt-output/rust.command.json");
    assert_eq!(all[0].event_ordinal, 0);
    assert_eq!(all[1].timestamp_ms, 1100);
    assert_eq!(all[2].timestamp_ms, 1200);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].artifact_path, "search/rust-search-owner.json");
    let _ = std::fs::remove_dir_all(root);
}

fn event(
    artifact_path: &str,
    event_ordinal: u32,
    timestamp_ms: i64,
    method: &str,
) -> ClientDbArtifactEvent {
    ClientDbArtifactEvent {
        artifact_path: artifact_path.to_string(),
        event_ordinal,
        timestamp_ms,
        kind: if artifact_path.starts_with("search/") {
            "search"
        } else {
            "command"
        }
        .to_string(),
        language: "rust".to_string(),
        method: method.to_string(),
        target: "src/lib.rs".to_string(),
        query: String::new(),
        project_root: "/tmp/project".to_string(),
        project_root_arg: ".".to_string(),
        bytes: 128,
    }
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
