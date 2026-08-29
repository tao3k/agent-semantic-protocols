use std::path::Path;

use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_db::ClientDbEngine;

use crate::search_history::history_audit::artifact_events_packet;
use crate::test_support::{CACHE_TEST_LOCK, EnvVarGuard};

#[tokio::test]
async fn search_history_rejects_unknown_subcommand() {
    let error = run_search_history(Path::new("."), &["owner".to_string()])
        .await
        .expect_err("unknown search history subcommand should fail");

    assert_eq!(
        error,
        "usage: asp search history audit [PROJECT_ROOT] [GRAPH_TURBO_TIMELINE_ARGS...]"
    );
}

#[tokio::test]
async fn search_history_backfills_artifacts_and_passes_db_engine_events() {
    let _guard = CACHE_TEST_LOCK.lock().expect("cache test lock");
    let root = temp_root("history-backfill");
    let _state_home = EnvVarGuard::set("ASP_STATE_HOME", root.join(".asp-state"));
    let artifact_dir = ResolvedState::resolve(&root)
        .expect("state core")
        .paths
        .artifacts_dir;
    std::fs::create_dir_all(artifact_dir.join("prompt-output")).expect("create artifact dir");
    std::fs::write(
        artifact_dir.join("prompt-output/rust-search-prime-abc123.txt"),
        "[search-prime] rust\n",
    )
    .expect("write artifact");
    std::fs::write(
        artifact_dir.join("prompt-output/rust-query-code-abc123.command.json"),
        r#"{
  "eventTimestampMs": 111111,
  "providerCommands": [
    {
      "startedAtMs": 222222,
      "argv": ["asp-rust", "query", "--selector", "src/lib.rs:1-10", "--projection", "source"],
      "languageId": "rust",
      "projectRoot": "__PROJECT_ROOT__"
    },
    {
      "eventTimestampMs": 333333,
      "argv": ["asp-rust", "query", "--selector", "src/main.rs:20-24", "--workspace", ".", "--projection", "source"],
      "languageId": "rust",
      "projectRoot": "__PROJECT_ROOT__"
    }
  ]
}"#
        .replace("__PROJECT_ROOT__", &root.display().to_string()),
    )
    .expect("write direct-read command artifact");
    std::fs::create_dir_all(artifact_dir.join("semantic-tree-sitter-query"))
        .expect("create syntax query artifact dir");
    std::fs::write(
        artifact_dir.join("semantic-tree-sitter-query/rust-query-tree-sitter-abc123.json"),
        r#"{
  "schemaId": "semantic-tree-sitter-query",
  "schemaVersion": "1",
  "eventTimestampMs": 555555,
  "languageId": "rust",
  "projectRoot": "__PROJECT_ROOT__",
  "method": "query",
  "query": {
    "input": "(function_item) @item",
    "inputForm": "s-expression"
  }
}"#
        .replace("__PROJECT_ROOT__", &root.display().to_string()),
    )
    .expect("write tree-sitter query artifact");
    let packet_bytes = artifact_events_packet(
        &agent_semantic_client_core::ProjectContext::resolve(&root).expect("project context"),
        &artifact_dir,
    )
    .expect("build event packet")
    .expect("complete event packet");
    let stdin = String::from_utf8(packet_bytes.to_vec()).expect("events packet utf8");
    assert!(stdin.contains("\"kind\":\"db-engine\""), "{stdin}");
    let packet: serde_json::Value = serde_json::from_str(&stdin).expect("events packet");
    let client_dir = packet
        .pointer("/source/clientDir")
        .and_then(serde_json::Value::as_str)
        .expect("client dir");
    let events = ClientDbEngine::lookup_artifact_events_from_client_dir(client_dir, None, 10)
        .expect("read db events");
    assert!(
        events
            .iter()
            .any(|event| event.artifact_path() == "prompt-output/rust-search-prime-abc123.txt"),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.artifact_path() == "prompt-output/rust-query-code-abc123.command.json"
                && event.method() == "query/source"
                && event.target() == "src/lib.rs:1-10"
                && event.timestamp_ms() == 222222
        }),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.artifact_path() == "prompt-output/rust-query-code-abc123.command.json"
                && event.method() == "query/source"
                && event.target() == "src/main.rs:20-24"
                && event.timestamp_ms() == 333333
        }),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.artifact_path() == "semantic-tree-sitter-query/rust-query-tree-sitter-abc123.json"
                && event.method() == "query/tree-sitter"
                && event.query() == "(function_item) @item"
                && event.timestamp_ms() == 555555
        }),
        "{events:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(name: &str) -> std::path::PathBuf {
    crate::test_support::owner_backed_temp_root(name)
}
