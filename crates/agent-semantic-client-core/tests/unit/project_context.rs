use std::fs;
use std::path::PathBuf;

use crate::ProjectContext;
use crate::StateLayout;
use crate::test_support::IsolatedAspStateHome;
use crate::test_support::init_durable_repo;

#[test]
fn project_context_resolves_git_toplevel_from_subdir() {
    let root = temp_root("git-toplevel");
    let _state_home = IsolatedAspStateHome::activate(&root);
    init_durable_repo(&root, "git-toplevel");
    let package = root.join("crates/example/src");
    fs::create_dir_all(&package).expect("create package dir");

    let context = ProjectContext::resolve(&package).expect("project context");

    assert_eq!(context.git_toplevel(), Some(root.as_path()));
    assert_eq!(context.project_home(), Some(root.as_path()));
    context.binding().validate().expect("typed project binding");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn state_layout_uses_single_client_cache_interface() {
    let root = temp_root("state-layout");
    let _state_home = IsolatedAspStateHome::activate(&root);
    init_durable_repo(&root, "state-layout");
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
fn project_context_resolution_is_pure_and_open_is_explicit() {
    let root = temp_root("pure-resolution");
    let state_home = root.join(".agent-semantic-protocols-test-state");
    let _isolated = IsolatedAspStateHome::activate(&root);
    init_durable_repo(&root, "pure-resolution");

    let context = ProjectContext::resolve(&root).expect("resolve project context");
    assert_eq!(context.state_layout().state_root(), state_home.as_path());
    assert!(
        !state_home.exists(),
        "pure resolution must not materialize State Home"
    );

    let opened = ProjectContext::open(&root).expect("open project context");
    assert!(opened.state_layout().client_cache_dir().is_dir());
    assert!(opened.state_layout().artifacts_dir().is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_boundary_rejects_paths_outside_git_toplevel() {
    let root = temp_root("workspace-boundary");
    let _state_home = IsolatedAspStateHome::activate(&root);
    init_durable_repo(&root, "workspace-boundary");
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
