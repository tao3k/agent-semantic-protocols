use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::Barrier;

use super::ensure_dir;
use super::project_root_for_activation_path;
use super::project_runtime_state_with_state_home;
use super::project_state_paths_with_state_home;
use super::provider_receipt_dir;

#[test]
fn runtime_state_materializes_state_core_layout() {
    let root = temp_root("runtime-state-git");
    let state_home = temp_root("runtime-state-home");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    init_git_repository(&root);

    let state =
        project_runtime_state_with_state_home(&package_root, &state_home).expect("runtime state");
    let resolved =
        crate::state_core::ResolvedState::resolve_with_state_home(&package_root, &state_home)
            .expect("resolved state layout");

    assert_eq!(resolved.repo.git_toplevel.as_deref(), Some(root.as_path()));
    assert_eq!(state.protocol_home, state_home);
    assert_eq!(state.hook_cache_dir, resolved.paths.hooks_dir.join("cache"));
    assert_eq!(state.hook_state_dir, resolved.paths.hooks_dir.join("state"));
    assert!(state_home.join("projects/by-id").exists());
    assert_eq!(
        state.activation_path,
        state.hook_state_dir.join("activation.json")
    );
    assert!(
        state
            .client_cache_dir
            .starts_with(state_home.join("projects/by-id"))
    );
    assert!(state.client_cache_dir.ends_with("live/client"));
    assert!(
        state
            .artifacts_dir
            .starts_with(state_home.join("projects/by-id"))
    );
    assert!(state.artifacts_dir.ends_with("artifacts"));
    assert_eq!(state.runtime_home, state_home.join("runtime"));
    let active_bundle =
        agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&state_home).active_slot();
    assert_eq!(state.provider_bin_dir, active_bundle);
    assert_eq!(state.runtime_bin_dir, active_bundle);
    assert_eq!(state.provider_lock_dir, provider_receipt_dir(&state_home));
    assert!(state.hook_cache_dir.is_dir());
    assert!(state.hook_state_dir.is_dir());
    assert!(state.client_cache_dir.is_dir());
    assert!(state.artifacts_dir.is_dir());
    assert!(state.runtime_home.is_dir());
    assert!(state.provider_bin_dir.is_dir());
    assert!(state.provider_lock_dir.is_dir());
    assert!(!root.join(".cache").exists());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn canonical_state_activation_path_resolves_workspace_root_without_legacy_fallback() {
    let root = temp_root("canonical-activation-root");
    let state_home = temp_root("canonical-activation-state-home");
    init_git_repository(&root);

    let state = project_runtime_state_with_state_home(&root, &state_home).expect("runtime state");
    fs::write(&state.activation_path, "{}\n").expect("write canonical activation");

    assert_eq!(
        project_root_for_activation_path(&state.activation_path),
        Some(root.clone())
    );

    let legacy = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    fs::create_dir_all(legacy.parent().expect("legacy activation parent"))
        .expect("create legacy activation parent");
    fs::write(&legacy, "{}\n").expect("write legacy activation");
    assert_eq!(
        project_root_for_activation_path(&legacy),
        None,
        "project-local activation layouts are not a second authority"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn project_state_path_resolution_never_materializes_project_directories() {
    let root = temp_root("state-paths-pure");
    let state_home = temp_root("state-paths-pure-home");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    init_git_repository(&root);

    for _ in 0..1_000 {
        let paths = crate::state::project_state_paths_with_state_home(&package_root, &state_home)
            .expect("resolve project state paths");
        assert!(paths.protocol_home.starts_with(&state_home));
    }

    assert!(
        !state_home.join("projects/by-id").exists(),
        "identity and path resolution must not materialize project directories"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn ordinary_non_git_roots_are_ephemeral_and_never_materialized() {
    let root = temp_root("state-non-git-ephemeral");
    let state_home = temp_root("state-non-git-ephemeral-home");
    let plugin_cache_like_root = root.join("plugins/cache");
    fs::create_dir_all(&plugin_cache_like_root).expect("create non-Git search root");

    for _ in 0..1_000 {
        let resolved = crate::state_core::ResolvedState::resolve_with_state_home(
            &plugin_cache_like_root,
            &state_home,
        )
        .expect("resolve ephemeral search root");
        assert_eq!(
            resolved.repo.persistence,
            crate::state_core::RepoPersistence::EphemeralPath
        );
        assert!(
            resolved.repo.identity_basis.starts_with("ephemeral-path:"),
            "path fallback must be explicitly ephemeral"
        );
    }

    let Err(error) = project_runtime_state_with_state_home(&plugin_cache_like_root, &state_home)
    else {
        panic!("an ordinary non-Git root must not own durable runtime state");
    };
    assert!(error.contains("refusing to materialize ephemeral or standalone temporary checkout"));
    assert!(
        !state_home.join("projects/by-id").exists(),
        "ephemeral resolution and rejected materialization must create no project directory"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn filesystem_git_marker_without_gix_repository_is_not_admitted() {
    let root = temp_root("state-fake-git-marker");
    let state_home = temp_root("state-fake-git-marker-home");
    fs::create_dir_all(root.join(".git")).expect("create unowned Git marker");

    let resolved = crate::state_core::ResolvedState::resolve_with_state_home(&root, &state_home)
        .expect("resolve fake Git marker as ephemeral");
    assert_eq!(
        resolved.repo.persistence,
        crate::state_core::RepoPersistence::EphemeralPath
    );
    let error = resolved
        .ensure_minimal_layout()
        .expect_err("filesystem marker must not bypass Gix admission");
    assert!(error.contains("refusing to materialize ephemeral or standalone temporary checkout"));
    assert!(!state_home.join("projects/by-id").exists());

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn standalone_temporary_git_repository_never_materializes_a_project_id() {
    let root = temp_root("state-standalone-temporary-git");
    let state_home = temp_root("state-standalone-temporary-git-home");
    let output = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&root)
        .output()
        .expect("initialize standalone temporary Git fixture");
    assert!(output.status.success());

    let resolved = crate::state_core::ResolvedState::resolve_with_state_home(&root, &state_home)
        .expect("resolve standalone temporary Git fixture");
    assert_eq!(
        resolved.repo.persistence,
        crate::state_core::RepoPersistence::EphemeralPath
    );
    let error = resolved
        .ensure_minimal_layout()
        .expect_err("standalone temporary Git fixture must not materialize");
    assert!(error.contains("standalone temporary checkout"));
    assert!(
        !state_home.join("projects/by-id").exists(),
        "rejected temporary Git checkout must create no project ID"
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn repeated_writers_in_one_checkout_materialize_one_repository_and_workspace() {
    let root = temp_root("state-writer-idempotence");
    let state_home = temp_root("state-writer-idempotence-home");
    let first_package = root.join("crates/first");
    let second_package = root.join("crates/second");
    fs::create_dir_all(&first_package).expect("create first package root");
    fs::create_dir_all(&second_package).expect("create second package root");
    init_git_repository(&root);

    let first = project_runtime_state_with_state_home(&first_package, &state_home)
        .expect("materialize first package state");
    for index in 0..32 {
        let package = if index % 2 == 0 {
            &first_package
        } else {
            &second_package
        };
        let repeated = project_runtime_state_with_state_home(package, &state_home)
            .expect("repeat project state writer");
        assert_eq!(repeated.client_cache_dir, first.client_cache_dir);
        assert_eq!(repeated.artifacts_dir, first.artifacts_dir);
    }

    let projects_by_id = state_home.join("projects/by-id");
    let repository_dirs = fs::read_dir(&projects_by_id)
        .expect("read repository directories")
        .map(|entry| entry.expect("read repository directory").path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    assert_eq!(
        repository_dirs.len(),
        1,
        "one checkout must materialize exactly one repository directory"
    );

    let workspace_count = fs::read_dir(repository_dirs[0].join("workspaces"))
        .expect("read workspace directories")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .count();
    assert_eq!(
        workspace_count, 1,
        "subdirectories in one checkout must share one workspace directory"
    );
    for repository_dir in &repository_dirs {
        assert!(
            repository_dir.join("project.json").is_file(),
            "every visible repository directory must have committed identity metadata"
        );
        for workspace in
            fs::read_dir(repository_dir.join("workspaces")).expect("read committed workspaces")
        {
            let workspace = workspace.expect("read committed workspace").path();
            if workspace.is_dir() {
                assert!(workspace.join("workspace.json").is_file());
                assert!(workspace.join("live/client/manifest.json").is_file());
            }
        }
    }
    assert!(
        fs::read_dir(&projects_by_id)
            .expect("read registry staging entries")
            .filter_map(Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().contains(".staging-")),
        "successful materialization must leave no staging directory"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn concurrent_writers_publish_one_complete_project_tree() {
    let root = temp_root("state-concurrent-writer");
    let state_home = temp_root("state-concurrent-writer-home");
    init_git_repository(&root);
    let resolved = Arc::new(
        crate::state_core::ResolvedState::resolve_with_state_home(&root, &state_home)
            .expect("resolve concurrent writer state"),
    );
    let writer_count = 8;
    let barrier = Arc::new(Barrier::new(writer_count));
    let mut writers = Vec::new();
    for _ in 0..writer_count {
        let resolved = Arc::clone(&resolved);
        let barrier = Arc::clone(&barrier);
        writers.push(std::thread::spawn(move || {
            barrier.wait();
            resolved.ensure_minimal_layout()
        }));
    }
    for writer in writers {
        writer
            .join()
            .expect("join concurrent State Core writer")
            .expect("concurrent State Core materialization");
    }

    assert!(resolved.paths.project_json.is_file());
    assert!(resolved.paths.workspace_json.is_file());
    assert!(resolved.paths.client_manifest_json.is_file());
    let workspaces_dir = resolved.paths.project_dir.join("workspaces");
    for directory in [&resolved.paths.projects_by_id_dir, &workspaces_dir] {
        assert!(
            fs::read_dir(directory)
                .expect("read State Core commit parent")
                .filter_map(Result::ok)
                .all(|entry| !entry.file_name().to_string_lossy().contains(".staging-")),
            "concurrent commit must leave no staging directory"
        );
    }

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn ensure_helpers_create_only_the_requested_runtime_dir() {
    let root = temp_root("runtime-state-single-dir");
    let state_home = temp_root("runtime-state-single-dir-home");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    init_git_repository(&root);

    let paths = project_state_paths_with_state_home(&package_root, &state_home)
        .expect("resolve project state paths");
    let resolved =
        crate::state_core::ResolvedState::resolve_with_state_home(&package_root, &state_home)
            .expect("resolved state layout");
    resolved
        .ensure_minimal_layout()
        .expect("materialize identity metadata");
    let hook_dir = ensure_dir(paths.hook_cache_dir).expect("hook cache dir");

    assert!(hook_dir.is_dir());
    assert_eq!(hook_dir, resolved.paths.hooks_dir.join("cache"));
    assert!(
        !hook_dir
            .parent()
            .expect("hook parent")
            .join("state")
            .exists()
    );
    let hook_state_dir = ensure_dir(paths.hook_state_dir).expect("hook state dir");
    let client_dir = ensure_dir(paths.client_cache_dir).expect("client cache dir");
    let runtime_home = ensure_dir(paths.runtime_home).expect("runtime home");
    let provider_bin_dir = ensure_dir(paths.provider_bin_dir).expect("provider bin dir");
    let provider_lock_dir = ensure_dir(paths.provider_lock_dir).expect("provider lock dir");

    assert!(hook_state_dir.is_dir());
    assert_eq!(
        hook_state_dir,
        hook_dir.parent().expect("hook parent").join("state")
    );
    assert!(client_dir.is_dir());
    assert!(client_dir.starts_with(state_home.join("projects/by-id")));
    assert!(client_dir.ends_with("live/client"));
    let workspace_dir = client_dir
        .parent()
        .and_then(|live_dir| live_dir.parent())
        .expect("workspace dir");
    let project_dir = workspace_dir
        .parent()
        .and_then(|workspaces_dir| workspaces_dir.parent())
        .expect("project dir");
    assert!(project_dir.join("project.json").is_file());
    assert!(workspace_dir.join("workspace.json").is_file());
    assert!(runtime_home.is_dir());
    assert!(provider_bin_dir.is_dir());
    assert!(provider_lock_dir.is_dir());
    assert!(!root.join(".cache").exists());
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-runtime-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

fn init_git_repository(root: &Path) {
    let output = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .output()
        .expect("run git init for Gix-owned test identity");
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            "https://example.invalid/asp/runtime-state-fixture.git",
        ])
        .current_dir(root)
        .output()
        .expect("add durable remote to State Core fixture");
    assert!(output.status.success(), "git remote add failed");
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
