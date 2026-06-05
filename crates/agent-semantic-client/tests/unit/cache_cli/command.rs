use crate::cache_cli::run_cache;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cache_flush_is_supported_as_explicit_cache_boundary_reset() {
    let root = temp_root("flush");
    std::fs::create_dir_all(&root).expect("mkdir temp root");

    run_cache(&root, &["flush".to_string()], false).expect("flush cache");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_usage_lists_flush() {
    let root = temp_root("usage");
    let error = run_cache(&root, &["unknown".to_string()], false).expect_err("usage");

    assert!(error.contains("status|import|invalidate|flush"));
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("agent-client-cache-cli-{label}-{nanos}"))
}
