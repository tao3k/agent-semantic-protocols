use std::{fs, path::PathBuf};

use crate::test_support::IsolatedAspStateHome;
use crate::{ProjectContext, StateLayout};

#[test]
fn project_context_resolves_git_toplevel_from_subdir() {
    let root = temp_root("git-toplevel");
    let _state_home = IsolatedAspStateHome::activate(&root);
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    let package = root.join("crates/example/src");
    fs::create_dir_all(&package).expect("create package dir");

    let context = ProjectContext::resolve(&package).expect("project context");

    assert_eq!(context.git_toplevel(), Some(root.as_path()));
    assert_eq!(context.project_home(), Some(root.as_path()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn state_layout_uses_single_client_cache_interface() {
    let root = temp_root("state-layout");
    let _state_home = IsolatedAspStateHome::activate(&root);
    fs::create_dir_all(root.join(".git")).expect("create git marker");
    let package = root.join("crates/example");
    fs::create_dir_all(&package).expect("create package dir");

    let layout = StateLayout::resolve(&package).expect("state layout");
    let resolved = crate::state_core::ResolvedState::resolve(&package).expect("resolved state");

    assert_eq!(layout.state_root(), resolved.state_home.as_path());
    assert_eq!(
        layout.client_cache_dir(),
        resolved.paths.client_dir.as_path()
    );
    assert_eq!(
        layout.cache_manifest_path(),
        resolved.paths.client_cache_manifest_path.as_path()
    );
    assert_eq!(
        layout.artifacts_dir(),
        resolved.paths.artifacts_dir.as_path()
    );
    assert!(!root.join(".cache").join("agent-semantic-protocol").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_boundary_rejects_paths_outside_git_toplevel() {
    let root = temp_root("workspace-boundary");
    let _state_home = IsolatedAspStateHome::activate(&root);
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
