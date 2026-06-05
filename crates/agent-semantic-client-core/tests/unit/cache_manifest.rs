use std::fs;
use std::path::PathBuf;

use crate::project_client_cache_dir;
use agent_semantic_runtime::PRJ_CACHE_HOME_ENV;

#[test]
fn package_root_uses_runtime_client_cache_root() {
    let root = temp_root("git-toplevel-cache-root");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    fs::write(
        package_root.join("Cargo.toml"),
        r#"[package]
name = "cache-root-fixture"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write manifest");

    let cache_dir = project_client_cache_dir(&package_root).expect("client cache dir");
    let expected_cache_home = std::env::var_os(PRJ_CACHE_HOME_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".cache"));

    assert_eq!(
        cache_dir,
        expected_cache_home.join("agent-semantic-protocol/client")
    );
    let _ = fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("agent-semantic-client-core-{label}-{nonce}"))
}
