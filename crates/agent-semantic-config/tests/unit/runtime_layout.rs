use std::fs;
use std::path::{Path, PathBuf};

use super::{
    ProjectCacheSource, ProjectEnvStatus, ProjectRuntimeEnv, project_cache_root_with_env,
    project_runtime_layout_with_env,
};

#[test]
fn git_toplevel_is_first_project_identity_for_workspace_packages() {
    let root = temp_root("runtime-layout-git-toplevel");
    let package_root = root.join("packages/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    fs::write(
        package_root.join("Cargo.toml"),
        "[package]\nname = \"example\"\n",
    )
    .expect("write package manifest");

    let layout = project_runtime_layout_with_env(&package_root, ProjectRuntimeEnv::default());
    let expected_cache_home = root.join(".cache");

    assert_eq!(layout.git_toplevel.as_deref(), Some(root.as_path()));
    assert_eq!(layout.project_home.as_deref(), Some(root.as_path()));
    assert_eq!(layout.project_env, ProjectEnvStatus::Unavailable);
    assert_eq!(
        layout.cache_home.as_deref(),
        Some(expected_cache_home.as_path())
    );
    assert_eq!(layout.cache_source, Some(ProjectCacheSource::GitToplevel));
    assert_eq!(
        layout.client_cache_dir,
        Some(root.join(".cache/agent-semantic-protocol/client"))
    );
    assert_eq!(
        layout.hook_cache_dir,
        Some(root.join(".cache/agent-semantic-protocol/hooks"))
    );
    assert_eq!(
        layout.hook_state_dir,
        Some(root.join(".cache/agent-semantic-protocol/hooks/state"))
    );
    assert_eq!(
        layout.activation_path,
        Some(root.join(".cache/agent-semantic-protocol/hooks/activation.json"))
    );
    assert_eq!(layout.artifacts_dir, None);
    assert_eq!(
        layout.runtime_home,
        Some(root.join(".cache/agent-semantic-protocol/runtime"))
    );
    assert_eq!(
        layout.runtime_bin_dir,
        Some(root.join(".cache/agent-semantic-protocol/runtime/bin"))
    );
    assert_eq!(
        layout.provider_lock_dir,
        Some(root.join(".cache/agent-semantic-protocol/runtime/providers"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_env_requires_envrc_at_git_toplevel() {
    let root = temp_root("runtime-layout-root-envrc");
    let package_root = root.join("packages/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    fs::write(root.join(".envrc"), "export PRJHOME=$PWD\n").expect("write root envrc");

    let layout = project_runtime_layout_with_env(&package_root, ProjectRuntimeEnv::default());

    assert_eq!(layout.project_home.as_deref(), Some(root.as_path()));
    assert_eq!(
        layout.project_env,
        ProjectEnvStatus::DirenvAtGitToplevel {
            envrc_path: root.join(".envrc")
        }
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn nested_envrc_does_not_enable_project_env() {
    let root = temp_root("runtime-layout-nested-envrc");
    let package_root = root.join("packages/example");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    fs::write(package_root.join(".envrc"), "export PRJHOME=$PWD\n").expect("write nested envrc");

    let layout = project_runtime_layout_with_env(&package_root, ProjectRuntimeEnv::default());

    assert_eq!(layout.project_home.as_deref(), Some(root.as_path()));
    assert_eq!(layout.project_env, ProjectEnvStatus::Unavailable);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn git_toplevel_precedes_prj_cache_home_for_state_storage() {
    let root = temp_root("runtime-layout-prj-cache-home");
    let package_root = root.join("packages/example");
    let state_root = root.join(".asp-state");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");

    let layout = project_runtime_layout_with_env(
        &package_root,
        ProjectRuntimeEnv {
            prj_cache_home: Some(state_root.clone()),
        },
    );

    assert_eq!(layout.git_toplevel.as_deref(), Some(root.as_path()));
    assert_eq!(layout.cache_home, Some(root.join(".cache")));
    assert_eq!(layout.cache_source, Some(ProjectCacheSource::GitToplevel));
    assert_eq!(layout.agents_dir, Some(root.join(".agents")));
    assert_eq!(
        layout.client_cache_dir,
        Some(root.join(".cache/agent-semantic-protocol/client"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn prj_cache_home_is_fallback_outside_git_worktree() {
    let root = temp_root("runtime-layout-prj-cache-home-fallback");
    let package_root = root.join("packages/example");
    let state_root = root.join(".asp-state");
    fs::create_dir_all(&package_root).expect("create package root");

    let layout = project_runtime_layout_with_env(
        &package_root,
        ProjectRuntimeEnv {
            prj_cache_home: Some(state_root.clone()),
        },
    );

    assert_eq!(layout.git_toplevel, None);
    assert_eq!(layout.prj_cache_home, Some(state_root.clone()));
    assert_eq!(layout.cache_source, Some(ProjectCacheSource::PrjCacheHome));
    assert_eq!(layout.cache_home, Some(state_root.clone()));
    assert_eq!(
        layout.activation_path,
        Some(state_root.join("agent-semantic-protocol/hooks/activation.json"))
    );
    assert_eq!(
        layout.hook_state_dir,
        Some(state_root.join("agent-semantic-protocol/hooks/state"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_path_helpers_require_git_or_prj_cache_home() {
    let root = temp_root("runtime-layout-no-cache-root");
    let package_root = root.join("packages/example");
    fs::create_dir_all(&package_root).expect("create package root");

    let error = project_cache_root_with_env(&package_root, ProjectRuntimeEnv::default())
        .expect_err("missing git and PRJ_CACHE_HOME must fail");

    assert!(error.contains("failed to locate ASP state root"));
    assert!(error.contains("PRJ_CACHE_HOME"));
    assert!(error.contains(&package_root.display().to_string()));
    let _ = fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-config-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
