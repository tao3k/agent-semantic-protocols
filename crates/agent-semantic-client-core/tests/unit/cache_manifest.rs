// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::path::PathBuf;

use crate::project_client_cache_dir;
use crate::test_support::IsolatedAspStateHome;
use crate::test_support::init_durable_repo;

#[test]
fn package_root_uses_git_toplevel_client_cache_root() {
    let root = temp_root("git-toplevel-cache-root");
    let _state_home = IsolatedAspStateHome::activate(&root);
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    init_durable_repo(&root, "git-toplevel-cache-root");
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
    let resolved =
        crate::state_core::ResolvedState::resolve(&package_root).expect("resolved state");
    let workspace = resolved.workspace_state_paths().expect("workspace paths");

    assert_eq!(cache_dir, workspace.root);
    assert!(!resolved.state_home.join("projects").exists());
    assert!(!root.join(".cache").join("agent-semantic-protocol").exists());
    let _ = fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-core-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    root.canonicalize().unwrap_or(root)
}
