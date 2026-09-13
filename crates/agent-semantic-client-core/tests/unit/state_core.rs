// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::state_core::DEFAULT_STATE_HOME_DIR;
use crate::state_core::ResolvedState;
use crate::state_core::STATE_LAYOUT_VERSION;
use crate::state_core::TURSO_BACKEND;
use crate::state_core::resolve_state_home_from;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[test]
fn state_home_prefers_asp_state_home() {
    let root = temp_root("home-prefers");
    let asp_state_home = root.join("custom-state");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();

    let resolved = resolve_state_home_from(
        Some(asp_state_home.clone().into_os_string()),
        Some(home.into_os_string()),
    )
    .unwrap();

    assert_eq!(
        resolved,
        fs::canonicalize(&root).unwrap().join("custom-state")
    );
}

#[test]
fn state_home_defaults_under_home() {
    let root = temp_root("home-default");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();

    let resolved = resolve_state_home_from(None, Some(home.clone().into_os_string())).unwrap();

    assert_eq!(
        resolved,
        fs::canonicalize(home).unwrap().join(DEFAULT_STATE_HOME_DIR)
    );
}

#[test]
fn same_display_name_does_not_collide() {
    let root = temp_root("display-collision");
    let state_home = root.join("state");
    let left = root.join("left").join("same-name");
    let right = root.join("right").join("same-name");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();

    let left_state = ResolvedState::resolve_with_state_home(&left, &state_home).unwrap();
    let right_state = ResolvedState::resolve_with_state_home(&right, &state_home).unwrap();

    assert_eq!(left_state.repo.display_name, "same-name");
    assert_eq!(right_state.repo.display_name, "same-name");
    assert_ne!(left_state.repo.repo_id, right_state.repo.repo_id);
    assert_ne!(
        left_state.workspace.workspace_id,
        right_state.workspace.workspace_id
    );
}

#[test]
fn canonical_layout_writes_binding_without_project_id_tree() {
    let root = durable_root("manifest");
    let work = root.join("work");
    let state_home = root.join("state");
    fs::create_dir_all(&work).unwrap();
    git(&work, &["init"]);
    git(
        &work,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/asp/minimal-layout.git",
        ],
    );

    let state = ResolvedState::resolve_with_state_home(&work, &state_home).unwrap();
    let workspace = state.ensure_workspace_state_layout().unwrap();

    assert!(workspace.binding_path().exists());
    assert!(workspace.artifacts.is_dir());
    assert!(workspace.observations.is_dir());
    assert!(!state_home.join("projects").exists());
    assert!(!work.join(".cache").join("agent-semantic-protocol").exists());

    let binding: agent_semantic_artifacts::ProjectBinding =
        serde_json::from_slice(&fs::read(workspace.binding_path()).unwrap()).unwrap();
    binding.validate().unwrap();
    assert_eq!(binding, state.project_binding().unwrap());

    let report = state.locate_report();
    assert_eq!(report.state_layout_version, STATE_LAYOUT_VERSION);
    assert_eq!(report.backend, TURSO_BACKEND);
    assert_eq!(report.db_path, workspace.facts);
    assert_eq!(report.artifact_path, workspace.artifacts);
    assert_eq!(report.manifest_path, workspace.db_manifest_path());
}

#[test]
fn state_locate_schema_declares_report_contract_fields() {
    let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("schemas")
        .join("semantic-state-locate-report.v1.schema.json");
    let schema: serde_json::Value =
        serde_json::from_slice(&fs::read(schema_path).unwrap()).unwrap();
    let required = schema["required"].as_array().expect("required array");

    for field in [
        "stateLayoutVersion",
        "stateHome",
        "repoId",
        "workspaceId",
        "scopeId",
        "repoDisplayName",
        "workspaceDisplayName",
        "checkoutRoot",
        "gitToplevel",
        "gitDir",
        "remoteUrl",
        "dbPath",
        "artifactPath",
        "manifestPath",
        "generationManifestPath",
        "backend",
        "projectLocalCache",
    ] {
        assert!(
            required.iter().any(|value| value.as_str() == Some(field)),
            "schema missing required field {field}"
        );
    }
}

#[test]
fn git_worktree_shares_repo_identity_but_not_workspace_identity() {
    let root = temp_root("git-worktree");
    let state_home = root.join("state");
    let main = root.join("repo");
    let worktree = root.join("repo-worktree");
    fs::create_dir_all(&main).unwrap();

    git(&main, &["init"]);
    git(
        &main,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/asp/git-worktree.git",
        ],
    );
    fs::write(main.join("README.md"), "state core\n").unwrap();
    git(&main, &["add", "README.md"]);
    git(
        &main,
        &[
            "-c",
            "user.email=asp@example.invalid",
            "-c",
            "user.name=ASP",
            "commit",
            "-m",
            "init",
        ],
    );
    git(&main, &["worktree", "add", worktree.to_str().unwrap()]);

    let main_state = ResolvedState::resolve_with_state_home(&main, &state_home).unwrap();
    let worktree_state = ResolvedState::resolve_with_state_home(&worktree, &state_home).unwrap();

    assert_eq!(main_state.repo.repo_id, worktree_state.repo.repo_id);
    assert_ne!(
        main_state.workspace.workspace_id,
        worktree_state.workspace.workspace_id
    );
    let main_binding = main_state.project_binding().unwrap();
    let worktree_binding = worktree_state.project_binding().unwrap();
    assert_eq!(main_binding.repo.digest, worktree_binding.repo.digest);
    assert_ne!(
        main_binding.workspace.digest,
        worktree_binding.workspace.digest
    );
    assert_ne!(
        main_binding.workspace.private_git_dir,
        worktree_binding.workspace.private_git_dir
    );
}

#[test]
fn git_remote_url_change_does_not_change_repo_identity() {
    let root = temp_root("git-remote-url-change");
    let state_home = root.join("state");
    let repo = root.join("repo");
    fs::create_dir_all(&repo).unwrap();

    git(&repo, &["init"]);
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "ssh://git@github.com/tao3k/agent-semantic-protocols.git",
        ],
    );
    let ssh_state = ResolvedState::resolve_with_state_home(&repo, &state_home).unwrap();

    git(
        &repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://mirror.example.invalid/fork/renamed-repository.git",
        ],
    );
    let https_state = ResolvedState::resolve_with_state_home(&repo, &state_home).unwrap();

    assert_eq!(ssh_state.repo.repo_id, https_state.repo.repo_id);
    assert_eq!(
        ssh_state.workspace.workspace_id,
        https_state.workspace.workspace_id
    );
    assert!(ssh_state.repo.identity_basis.starts_with("git-common-dir:"));
    assert!(
        https_state
            .repo
            .identity_basis
            .starts_with("git-common-dir:")
    );
}

#[test]
fn independent_clones_with_the_same_remote_do_not_share_mutable_state_identity() {
    let root = temp_root("independent-clones-same-remote");
    let state_home = root.join("state");
    let first = root.join("first-clone");
    let second = root.join("second-clone");
    for checkout in [&first, &second] {
        fs::create_dir_all(checkout).unwrap();
        git(checkout, &["init"]);
        git(
            checkout,
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/asp/shared-upstream.git",
            ],
        );
    }

    let first_state = ResolvedState::resolve_with_state_home(&first, &state_home).unwrap();
    let second_state = ResolvedState::resolve_with_state_home(&second, &state_home).unwrap();

    assert_ne!(first_state.repo.repo_id, second_state.repo.repo_id);
    assert_ne!(
        first_state.workspace.workspace_id,
        second_state.workspace.workspace_id
    );
}

fn durable_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/state-core-fixtures")
        .join(format!("{label}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "asp-state-core-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn git(cwd: &Path, args: &[&str]) {
    let output = Command::new(git_binary())
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_binary() -> PathBuf {
    for candidate in ["git", "/usr/bin/git", "/opt/homebrew/bin/git"] {
        if Command::new(candidate).arg("--version").output().is_ok() {
            return PathBuf::from(candidate);
        }
    }
    panic!("git binary is required for State Core worktree identity tests");
}
