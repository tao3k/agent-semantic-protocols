use crate::cache_cli::run_cache;
use agent_semantic_client_core::ClientCacheManifest;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cache_usage_lists_flush() {
    let root = temp_root("usage");
    let error = run_cache(&root, &["unknown".to_string()], false).expect_err("usage");

    assert!(error.contains("status|import|invalidate|flush [syntax-rows]"));
}

#[test]
fn cache_flush_repairs_invalid_manifest() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("invalid-manifest");
    let cache_report = ClientCacheManifest::inspect_project(&root);
    let manifest_path = cache_report.manifest_path.clone().expect("manifest path");
    std::fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create cache dir");
    std::fs::write(&manifest_path, "{}\n{}\n").expect("write invalid manifest");

    run_cache(&root, &["flush".to_string()], false).expect("flush invalid manifest");

    let manifest = ClientCacheManifest::load_from_path(&manifest_path).expect("repaired manifest");
    assert!(manifest.generations.is_empty());
    let _ = std::fs::remove_dir_all(root);
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
