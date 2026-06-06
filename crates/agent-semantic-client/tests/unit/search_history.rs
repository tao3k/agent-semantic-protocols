use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::ClientDb;

use crate::search_history::run_search_history;

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
    let root = temp_root("history-backfill");
    let artifact_dir = root.join(".cache/agent-semantic-protocol/artifacts");
    std::fs::create_dir_all(artifact_dir.join("prompt-output")).expect("create artifact dir");
    std::fs::write(
        artifact_dir.join("prompt-output/rust-search-prime-abc123.txt"),
        "[search-prime] rust\n",
    )
    .expect("write artifact");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let stdin_path = root.join("graph-turbo-stdin.json");
    let args_path = root.join("graph-turbo-args.txt");
    let graph_turbo = bin_dir.join("graph-turbo");
    std::fs::write(
        &graph_turbo,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$GRAPH_TURBO_ARGS_OUT\"\ncat > \"$GRAPH_TURBO_STDIN_OUT\"\nprintf '[graph-turbo-test]\\n'\n",
    )
    .expect("write graph-turbo");
    make_executable(&graph_turbo);

    let previous_path = std::env::var_os("PATH");
    let previous_prj_cache_home = std::env::var_os("PRJ_CACHE_HOME");
    let cache_root = root.join(".cache/agent-semantic-protocol");
    unsafe {
        std::env::set_var("PATH", prepend_path(&bin_dir));
        std::env::set_var("GRAPH_TURBO_STDIN_OUT", &stdin_path);
        std::env::set_var("GRAPH_TURBO_ARGS_OUT", &args_path);
        std::env::set_var("PRJ_CACHE_HOME", &cache_root);
    }

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

    match previous_path {
        Some(path) => unsafe {
            std::env::set_var("PATH", path);
        },
        None => unsafe {
            std::env::remove_var("PATH");
        },
    }
    unsafe {
        std::env::remove_var("GRAPH_TURBO_STDIN_OUT");
        std::env::remove_var("GRAPH_TURBO_ARGS_OUT");
    }
    match previous_prj_cache_home {
        Some(path) => unsafe {
            std::env::set_var("PRJ_CACHE_HOME", path);
        },
        None => unsafe {
            std::env::remove_var("PRJ_CACHE_HOME");
        },
    }

    result.expect("run search history");
    let args = std::fs::read_to_string(&args_path).expect("read graph-turbo args");
    let stdin = std::fs::read_to_string(&stdin_path).expect("read graph-turbo stdin");
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
