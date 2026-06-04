use std::fs;
use std::path::PathBuf;

use crate::project_client_cache_dir;

#[test]
fn package_manifest_root_owns_client_cache_without_local_activation() {
    let root = temp_root("package-cache-root");
    fs::create_dir_all(&root).expect("create temp root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "cache-root-fixture"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write manifest");

    let cache_dir = project_client_cache_dir(&root).expect("client cache dir");

    assert_eq!(
        cache_dir,
        root.join(".cache/agent-semantic-protocol/client")
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
