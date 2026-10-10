// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Owner-backed filesystem fixtures shared by database-engine scenarios.

use std::path::Path;
use std::path::PathBuf;

pub(super) fn temp_root(label: &str) -> PathBuf {
    let repository = gix::discover(env!("CARGO_MANIFEST_DIR"))
        .expect("discover owner-backed database test repository with Gix");
    let mut root = repository
        .worktree()
        .expect("database tests require a non-bare owner checkout")
        .base()
        .join("target/asp-live-project-fixtures");
    let unique = format!(
        "asp-client-db-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    );
    root.push(unique);
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

pub(super) fn init_git_repository(root: &Path) {
    let repository = gix::discover(root).expect("resolve owner repository with Gix");
    assert!(
        repository.worktree().is_some(),
        "database fixture must remain inside an owner-backed worktree"
    );
}
