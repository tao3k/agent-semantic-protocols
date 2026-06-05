use crate::cache_cli::run_cache;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cache_usage_lists_flush() {
    let root = temp_root("usage");
    let error = run_cache(&root, &["unknown".to_string()], false).expect_err("usage");

    assert!(error.contains("status|import|invalidate|flush [syntax-rows]"));
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-client-cache-cli-{label}-{nanos}"));
    std::fs::create_dir_all(root.join(".git")).expect("create temp project root");
    root
}
