use std::fs;
use std::path::{Path, PathBuf};

use super::{
    ensure_dir, project_root_for_activation_path, project_runtime_state_with_state_home,
    project_state_paths_with_state_home,
};

#[test]
fn runtime_state_materializes_config_layout_under_git_toplevel() {
    let root = temp_root("runtime-state-git");
    let state_home = temp_root("runtime-state-home");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

    let state =
        project_runtime_state_with_state_home(&package_root, &state_home).expect("runtime state");
    let resolved =
        crate::state_core::ResolvedState::resolve_with_state_home(&package_root, &state_home)
            .expect("resolved state layout");

    assert_eq!(state.layout.git_toplevel.as_deref(), Some(root.as_path()));
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
    assert_eq!(state.provider_bin_dir, state_home.join("runtime/bin"));
    assert_eq!(state.runtime_bin_dir, state_home.join("runtime/bin"));
    assert_eq!(
        state.provider_lock_dir,
        state_home.join("runtime/provider-locks")
    );
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
    fs::create_dir_all(root.join(".git")).expect("create git marker");

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
    fs::create_dir_all(root.join(".git")).expect("create git marker");

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
    assert!(error.contains("refusing to materialize ephemeral non-Git search root"));
    assert!(
        !state_home.join("projects/by-id").exists(),
        "ephemeral resolution and rejected materialization must create no project directory"
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
    fs::create_dir_all(root.join(".git")).expect("create git marker");

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
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn project_state_path_resolution_never_materializes_project_directories() {
    let root = temp_root("state-paths-pure");
    let state_home = AspStateHomeGuard::new("state-paths-pure-home");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

    for _ in 0..1_000 {
        let paths =
            crate::state::project_state_paths(&package_root).expect("resolve project state paths");
        assert!(paths.protocol_home.starts_with(state_home.path()));
    }

    assert!(
        !state_home.path().join("projects/by-id").exists(),
        "identity and path resolution must not materialize project directories"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ordinary_non_git_roots_are_ephemeral_and_never_materialized() {
    let root = temp_root("state-non-git-ephemeral");
    let state_home = AspStateHomeGuard::new("state-non-git-ephemeral-home");
    let plugin_cache_like_root = root.join("plugins/cache");
    fs::create_dir_all(&plugin_cache_like_root).expect("create non-Git search root");

    for _ in 0..1_000 {
        let resolved = crate::state_core::ResolvedState::resolve_with_state_home(
            &plugin_cache_like_root,
            state_home.path(),
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

    let Err(error) = project_runtime_state(&plugin_cache_like_root) else {
        panic!("an ordinary non-Git root must not own durable runtime state");
    };
    assert!(error.contains("refusing to materialize ephemeral non-Git search root"));
    assert!(
        !state_home.path().join("projects/by-id").exists(),
        "ephemeral resolution and rejected materialization must create no project directory"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repeated_writers_in_one_checkout_materialize_one_repository_and_workspace() {
    let root = temp_root("state-writer-idempotence");
    let state_home = AspStateHomeGuard::new("state-writer-idempotence-home");
    let first_package = root.join("crates/first");
    let second_package = root.join("crates/second");
    fs::create_dir_all(&first_package).expect("create first package root");
    fs::create_dir_all(&second_package).expect("create second package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

    let first = project_runtime_state(&first_package).expect("materialize first package state");
    for index in 0..32 {
        let package = if index % 2 == 0 {
            &first_package
        } else {
            &second_package
        };
        let repeated = project_runtime_state(package).expect("repeat project state writer");
        assert_eq!(repeated.client_cache_dir, first.client_cache_dir);
        assert_eq!(repeated.artifacts_dir, first.artifacts_dir);
    }

    let projects_by_id = state_home.path().join("projects/by-id");
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
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_helpers_create_only_the_requested_runtime_dir() {
    let root = temp_root("runtime-state-single-dir");
    let state_home = temp_root("runtime-state-single-dir-home");
    let package_root = root.join("crates/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

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

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
