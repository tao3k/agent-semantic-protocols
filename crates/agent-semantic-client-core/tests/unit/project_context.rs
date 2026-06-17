use std::{fs, path::PathBuf};

use agent_semantic_config::project_runtime_layout;

use crate::{ProjectContext, ProjectEnvStatus, StateLayout};

#[test]
fn project_context_resolves_git_toplevel_from_subdir() {
    let root = temp_root("git-toplevel");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    let package = root.join("crates/example/src");
    fs::create_dir_all(&package).expect("create package dir");

    let context = ProjectContext::resolve(&package).expect("project context");

    assert_eq!(context.git_toplevel(), Some(root.as_path()));
    assert_eq!(context.project_home(), Some(root.as_path()));
    assert_eq!(context.project_env(), &ProjectEnvStatus::Unavailable);
    assert!(!context.prj_env_vars_available());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_env_vars_require_envrc_at_git_toplevel() {
    let root = temp_root("envrc-at-root");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    fs::write(root.join(".envrc"), "export PRJHOME=$PWD\n").expect("write envrc");
    let package = root.join("crates/example");
    fs::create_dir_all(&package).expect("create package dir");

    let context = ProjectContext::resolve(&package).expect("project context");

    assert!(context.prj_env_vars_available());
    assert_eq!(
        context.project_env(),
        &ProjectEnvStatus::DirenvAtGitToplevel {
            envrc_path: root.join(".envrc")
        }
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn nested_envrc_without_git_toplevel_envrc_does_not_enable_prj_vars() {
    let root = temp_root("nested-envrc");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    let package = root.join("crates/example");
    fs::create_dir_all(&package).expect("create package dir");
    fs::write(package.join(".envrc"), "export PRJHOME=$PWD\n").expect("write nested envrc");

    let context = ProjectContext::resolve(&package).expect("project context");

    assert_eq!(context.project_home(), Some(root.as_path()));
    assert_eq!(context.project_env(), &ProjectEnvStatus::Unavailable);
    assert!(!context.prj_env_vars_available());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn state_layout_uses_single_client_cache_interface() {
    let root = temp_root("state-layout");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    let package = root.join("crates/example");
    fs::create_dir_all(&package).expect("create package dir");

    let layout = StateLayout::resolve(&package).expect("state layout");

    assert_eq!(
        layout.state_root(),
        root.join(".cache/agent-semantic-protocol").as_path()
    );
    assert_eq!(
        layout.client_cache_dir(),
        root.join(".cache/agent-semantic-protocol/client").as_path()
    );
    assert_eq!(
        layout.cache_manifest_path(),
        root.join(".cache/agent-semantic-protocol/client/cache-manifest.json")
            .as_path()
    );
    assert_eq!(
        layout.artifacts_dir(),
        root.join(".cache/agent-semantic-protocol/artifacts")
            .as_path()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn state_layout_matches_config_runtime_layout() {
    let root = temp_root("config-runtime-layout");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    let layout = project_runtime_layout(&root);
    let state_layout = StateLayout::resolve(&root).expect("state layout");

    assert_eq!(
        Some(state_layout.state_root()),
        layout.protocol_home.as_deref()
    );
    assert_eq!(
        Some(state_layout.client_cache_dir()),
        layout.client_cache_dir.as_deref()
    );
    assert_eq!(
        Some(state_layout.artifacts_dir()),
        layout.artifacts_dir.as_deref()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_boundary_rejects_paths_outside_git_toplevel() {
    let root = temp_root("workspace-boundary");
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    let inside = root.join("src/lib.rs");
    fs::create_dir_all(inside.parent().expect("inside parent")).expect("create src");
    fs::write(&inside, "").expect("write inside file");
    let outside = temp_root("outside-boundary").join("other.rs");
    fs::write(&outside, "").expect("write outside file");

    let context = ProjectContext::resolve(&root).expect("project context");

    assert_eq!(
        context
            .require_inside_workspace(&inside)
            .expect("inside workspace"),
        inside
    );
    assert!(
        context
            .require_inside_workspace(&outside)
            .expect_err("outside rejected")
            .contains("outside workspace")
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(outside);
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
