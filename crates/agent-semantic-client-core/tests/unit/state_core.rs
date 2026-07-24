use crate::state_core::ResolvedState;
use crate::state_core::{
    DEFAULT_STATE_HOME_DIR, STATE_LAYOUT_VERSION, TURSO_BACKEND, resolve_state_home_from,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn minimal_layout_writes_manifest_without_project_cache() {
    let root = temp_root("manifest");
    let work = root.join("work");
    let state_home = root.join("state");
    fs::create_dir_all(&work).unwrap();

    let state = ResolvedState::resolve_with_state_home(&work, &state_home).unwrap();
    state.ensure_minimal_layout().unwrap();

    assert!(state.paths.version_file.exists());
    assert!(state.paths.state_json.exists());
    assert!(state.paths.registry_events_jsonl.exists());
    assert!(state.paths.project_json.exists());
    assert!(state.paths.workspace_json.exists());
    assert!(state.paths.client_manifest_json.exists());
    assert!(state.paths.artifacts_dir.is_dir());
    assert!(!work.join(".cache").join("agent-semantic-protocol").exists());

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&state.paths.client_manifest_json).unwrap()).unwrap();
    assert_eq!(manifest["stateLayoutVersion"], STATE_LAYOUT_VERSION);
    assert_eq!(manifest["backend"], TURSO_BACKEND);
    assert_eq!(
        manifest["repoId"].as_str(),
        Some(state.repo.repo_id.as_str())
    );
    assert_eq!(
        manifest["workspaceId"].as_str(),
        Some(state.workspace.workspace_id.as_str())
    );
    assert_eq!(
        manifest["dbPath"].as_str(),
        Some(state.paths.client_db_path.to_str().unwrap())
    );
    assert_eq!(
        manifest["artifactPath"].as_str(),
        Some(state.paths.artifacts_dir.to_str().unwrap())
    );

    let report = state.locate_report();
    assert_eq!(report.state_layout_version, STATE_LAYOUT_VERSION);
    assert_eq!(report.backend, TURSO_BACKEND);
    assert_eq!(report.db_path, state.paths.client_db_path);
    assert_eq!(report.artifact_path, state.paths.artifacts_dir);
    assert_eq!(report.manifest_path, state.paths.client_manifest_json);
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
            "https://github.com/tao3k/agent-semantic-protocols.git",
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
