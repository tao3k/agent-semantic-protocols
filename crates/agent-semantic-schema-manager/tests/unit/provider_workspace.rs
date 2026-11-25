// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::process::Command;

use tempfile::TempDir;

use crate::{ProviderCheckoutState, inspect_provider_workspace, lock_provider_workspace};

fn run_git(dir: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn gix_inspects_and_locks_provider_workspace_without_cli() {
    let workspace = TempDir::new().expect("temp workspace");
    let pkg_dir = workspace.path().join("languages/asp-fixture");
    std::fs::create_dir_all(&pkg_dir).expect("create package dir");

    run_git(&pkg_dir, &["init", "-q"]);
    run_git(&pkg_dir, &["config", "user.email", "test@tao3k.invalid"]);
    run_git(&pkg_dir, &["config", "user.name", "ASP Test"]);
    std::fs::write(pkg_dir.join("README.md"), "fixture\n").expect("write readme");
    run_git(&pkg_dir, &["add", "README.md"]);
    run_git(&pkg_dir, &["commit", "-q", "-m", "initial fixture"]);
    let head_rev = run_git(&pkg_dir, &["rev-parse", "HEAD"]);

    let stale_rev = "0".repeat(40);
    std::fs::write(
        workspace.path().join("providers.toml"),
        format!(
            "schema_version = 1\n\
             [profiles]\n\
             core = [\"asp-fixture\"]\n\
             [packages.asp-fixture]\n\
             kind = \"language-provider\"\n\
             language = \"fixture\"\n\
             path = \"languages/asp-fixture\"\n\
             repo = \"https://github.com/tao3k/asp-fixture\"\n\
             rev = \"{stale_rev}\"\n\
             transport = \"grpc\"\n\
             visibility = \"public\"\n\
             optional = false\n"
        ),
    )
    .expect("write providers.toml");

    let drifted = inspect_provider_workspace(workspace.path(), Some("core"), &[])
        .expect("inspect drifted workspace via gix");
    assert_eq!(drifted.len(), 1);
    assert_eq!(drifted[0].state, ProviderCheckoutState::Drifted);
    assert_eq!(drifted[0].actual_rev.as_deref(), Some(head_rev.as_str()));
    assert!(!drifted[0].dirty);

    let locked = lock_provider_workspace(workspace.path(), Some("core"), &[])
        .expect("lock workspace via gix");
    assert_eq!(locked.len(), 1);
    assert_eq!(locked[0].1.as_str(), head_rev);

    let synced = inspect_provider_workspace(workspace.path(), Some("core"), &[])
        .expect("inspect synced workspace via gix");
    assert_eq!(synced[0].state, ProviderCheckoutState::Synced);
    assert!(!synced[0].dirty);
}
