// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! State Home binding tests.

use std::path::Path;

use crate::HostProjectReference;
use crate::ProjectBinding;

#[test]
fn host_reference_is_optional_and_never_becomes_workspace_identity() {
    let root = Path::new("/tmp/project-binding-fixture");
    let projectless =
        ProjectBinding::resolve(None, "git-common-dir:/workspace/.git", root).unwrap();
    let host_bound = ProjectBinding::resolve(
        Some(HostProjectReference {
            platform: "codex".to_string(),
            project_id: "local-host-id".to_string(),
            project_kind: "local".to_string(),
            host_id: Some("local".to_string()),
        }),
        "git-common-dir:/workspace/.git",
        root,
    )
    .unwrap();

    assert_eq!(projectless.repo, host_bound.repo);
    assert_eq!(projectless.workspace, host_bound.workspace);
    assert_ne!(projectless.binding_digest, host_bound.binding_digest);
    projectless.validate().unwrap();
    host_bound.validate().unwrap();
}

#[test]
fn full_content_identity_is_not_a_truncated_path_token() {
    let binding = ProjectBinding::resolve(None, "repo", "/tmp/workspace").unwrap();
    assert_eq!(binding.repo.digest.as_str().len(), "blake3-256:".len() + 64);
    assert_eq!(
        binding.workspace.digest.as_str().len(),
        "blake3-256:".len() + 64
    );
    assert_eq!(
        binding.binding_digest.as_str().len(),
        "blake3-256:".len() + 64
    );
}

#[test]
fn linked_worktrees_share_repo_identity_but_not_workspace_identity() {
    let root = tempfile::tempdir().unwrap();
    let common = root.path().join("repo.git");
    let left_root = root.path().join("left");
    let right_root = root.path().join("right");
    let left_git = common.join("worktrees/left");
    let right_git = common.join("worktrees/right");
    for path in [&common, &left_root, &right_root, &left_git, &right_git] {
        std::fs::create_dir_all(path).unwrap();
    }

    let left = ProjectBinding::resolve_with_private_git_dir(
        None,
        common.display().to_string(),
        &left_root,
        Some(&left_git),
    )
    .unwrap();
    let right = ProjectBinding::resolve_with_private_git_dir(
        None,
        common.display().to_string(),
        &right_root,
        Some(&right_git),
    )
    .unwrap();

    assert_eq!(left.repo.digest, right.repo.digest);
    assert_ne!(left.workspace.digest, right.workspace.digest);
    assert_eq!(
        left.workspace.private_git_dir.as_deref(),
        Some(left_git.canonicalize().unwrap().as_path())
    );
    assert_eq!(
        right.workspace.private_git_dir.as_deref(),
        Some(right_git.canonicalize().unwrap().as_path())
    );
}
