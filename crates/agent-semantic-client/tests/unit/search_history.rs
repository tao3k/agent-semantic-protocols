use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_db::ClientDb;

use crate::search_history::run_search_history;
use crate::test_support::{CACHE_TEST_LOCK, EnvVarGuard};

#[test]
fn search_history_rejects_unknown_subcommand() {
    let error = run_search_history(Path::new("."), &["owner".to_string()])
        .expect_err("unknown search history subcommand should fail");

    assert_eq!(
        error,
        "usage: asp search history audit [PROJECT_ROOT] [GRAPH_TURBO_TIMELINE_ARGS...]"
    );
}

#[test]
fn search_history_backfills_artifacts_and_passes_rust_sqlite_events() {
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
        artifact_dir.join("prompt-output/rust-query-direct-source-read-abc123.command.json"),
        r#"{
  "eventTimestampMs": 111111,
  "providerCommands": [
    {
      "startedAtMs": 222222,
      "argv": ["rs-harness", "query", "--from-hook", "direct-source-read", "--selector", "src/lib.rs:1-10", "--code"],
      "languageId": "rust"
    },
    {
      "eventTimestampMs": 333333,
      "argv": ["rs-harness", "query", "--selector", "src/main.rs:20-24", "--workspace", ".", "--code"],
      "languageId": "rust"
    }
  ]
}"#,
    )
    .expect("write direct-read command artifact");
    std::fs::create_dir_all(artifact_dir.join("analysis-metadata"))
        .expect("create analysis metadata artifact dir");
    std::fs::write(
        artifact_dir.join("analysis-metadata/rust-search-prime-abc123.json"),
        r#"{
  "schemaId": "agent.semantic-protocols.client-history-analysis-metadata",
  "schemaVersion": "1",
  "protocolId": "agent.semantic-protocols.client",
  "protocolVersion": "1",
  "eventTimestampMs": 444444,
  "sourceArtifactId": "prompt-output/rust-search-prime-abc123.txt",
  "sourceArtifactKind": "prompt-output",
  "languageId": "rust",
  "providerId": "rs-harness",
  "projectRoot": ".",
  "method": "search/prime",
  "exportMethod": "search/prime",
  "query": "ownerCandidates=src/lib.rs",
  "target": "",
  "developerMode": {
    "defaultEnabled": true,
    "storageOnly": true
  },
  "agentFacingOutput": {
    "unchanged": true,
    "metadataSurface": "history-analysis"
  },
  "request": {
    "method": "search",
    "languageId": "rust",
    "forwardedArgs": ["prime", "--view", "seeds", "."]
  },
  "artifact": {
    "bytes": 20,
    "fnv64": "0000000000000001"
  },
  "output": {
    "bytes": 20,
    "lineCount": 1,
    "fnv64": "0000000000000001"
  },
  "analysis": {
    "recognizedLineCount": 1,
    "fieldLines": {
      "ownerCandidates": "src/lib.rs"
    }
  }
}"#,
    )
    .expect("write analysis metadata artifact");
    std::fs::create_dir_all(artifact_dir.join("semantic-tree-sitter-query"))
        .expect("create syntax query artifact dir");
    std::fs::write(
        artifact_dir.join("semantic-tree-sitter-query/rust-query-tree-sitter-abc123.json"),
        r#"{
  "schemaId": "semantic-tree-sitter-query",
  "schemaVersion": "1",
  "eventTimestampMs": 555555,
  "languageId": "rust",
  "method": "query",
  "query": {
    "input": "(function_item) @item",
    "inputForm": "s-expression"
  }
}"#,
    )
    .expect("write tree-sitter query artifact");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let stdin_path = root.join("asp-graph-turbo-stdin.json");
    let args_path = root.join("asp-graph-turbo-args.txt");
    let graph_turbo = bin_dir.join("asp-graph-turbo");
    std::fs::write(
        &graph_turbo,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$ASP_GRAPH_TURBO_ARGS_OUT\"\ncat > \"$ASP_GRAPH_TURBO_STDIN_OUT\"\nprintf '[graph-turbo-test]\\n'\n",
    )
    .expect("write asp-graph-turbo");
    make_executable(&graph_turbo);

    let cache_root = root.join(".cache");
    let _path = EnvVarGuard::set("PATH", prepend_path(&bin_dir));
    let _stdin = EnvVarGuard::set("ASP_GRAPH_TURBO_STDIN_OUT", &stdin_path);
    let _args = EnvVarGuard::set("ASP_GRAPH_TURBO_ARGS_OUT", &args_path);
    let _ignored_cache_home = EnvVarGuard::set("PRJ_CACHE_HOME", &cache_root);

    let result = run_search_history(
        &root,
        &[
            "history".to_string(),
            "audit".to_string(),
            ".".to_string(),
            "--recent-sessions".to_string(),
            "1".to_string(),
        ],
    );

    result.expect("run search history");
    let args = std::fs::read_to_string(&args_path).expect("read asp-graph-turbo args");
    let stdin = std::fs::read_to_string(&stdin_path).expect("read asp-graph-turbo stdin");
    assert!(args.contains("--events-json"), "{args}");
    assert!(stdin.contains("\"kind\":\"rust-sqlite\""), "{stdin}");
    let packet: serde_json::Value = serde_json::from_str(&stdin).expect("events packet");
    let db_path = packet
        .pointer("/source/dbPath")
        .and_then(serde_json::Value::as_str)
        .expect("db path");
    let events = ClientDb::lookup_artifact_events(db_path, None, 10).expect("read db events");
    assert!(
        events
            .iter()
            .any(|event| event.artifact_path == "prompt-output/rust-search-prime-abc123.txt"),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.artifact_path == "prompt-output/rust-query-direct-source-read-abc123.command.json"
                && event.method == "query/direct-source-read"
                && event.target == "src/lib.rs:1-10"
                && event.timestamp_ms == 222222
        }),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.artifact_path == "prompt-output/rust-query-direct-source-read-abc123.command.json"
                && event.method == "query/code"
                && event.target == "src/main.rs:20-24"
                && event.timestamp_ms == 333333
        }),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.artifact_path == "analysis-metadata/rust-search-prime-abc123.json"
                && event.kind == "analysis-metadata"
                && event.method == "search/prime"
                && event.query == "ownerCandidates=src/lib.rs"
                && event.timestamp_ms == 444444
        }),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| {
            event.artifact_path == "semantic-tree-sitter-query/rust-query-tree-sitter-abc123.json"
                && event.method == "query/tree-sitter"
                && event.query == "(function_item) @item"
                && event.timestamp_ms == 555555
        }),
        "{events:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn prepend_path(bin_dir: &Path) -> std::ffi::OsString {
    let mut paths = vec![bin_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths).expect("join PATH")
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("chmod");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
