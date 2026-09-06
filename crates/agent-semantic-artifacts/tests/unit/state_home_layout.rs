//! State Home layout tests.

use crate::ProjectBinding;
use crate::StateHomeLayout;

#[test]
fn physical_namespace_has_no_schema_version_suffix() {
    let binding = ProjectBinding::resolve(None, "repo", "/tmp/workspace").unwrap();
    let layout = StateHomeLayout::new("/tmp/state-home");
    let workspace = layout.workspace(&binding.workspace).unwrap();
    let runtime_root = layout.runtime_state().root().to_path_buf();

    for path in [
        &layout.catalog,
        &layout.workspaces,
        &runtime_root,
        &layout.receipts,
        &layout.trash,
        &workspace.root,
    ] {
        let rendered = path.to_string_lossy();
        assert!(!rendered.contains("state-v"));
        assert!(!rendered.contains("/v1"));
        assert!(!rendered.contains("/v2"));
    }
}

#[test]
fn runtime_generated_state_is_derived_by_one_typed_layout() {
    let layout = StateHomeLayout::new("/tmp/state-home");
    let runtime = layout.runtime_state();

    assert_eq!(
        runtime.root(),
        std::path::Path::new("/tmp/state-home/runtime")
    );
    assert_eq!(
        runtime.artifacts().root(),
        std::path::Path::new("/tmp/state-home/runtime/artifacts")
    );
    assert_eq!(
        runtime.serving().root(),
        std::path::Path::new("/tmp/state-home/runtime/serving")
    );
    assert_eq!(
        runtime.serving().endpoint_receipt(),
        std::path::PathBuf::from("/tmp/state-home/runtime/serving/endpoint.v1.json")
    );
    assert_eq!(
        runtime.serving().readiness(),
        std::path::PathBuf::from("/tmp/state-home/runtime/serving/readiness")
    );
    assert_eq!(
        runtime.serving().workspaces(),
        std::path::PathBuf::from("/tmp/state-home/runtime/serving/workspaces")
    );
}

#[test]
fn runtime_status_memory_is_content_bound_and_deterministic() {
    let serving = StateHomeLayout::new("/tmp/state-home")
        .runtime_state()
        .serving();
    let first = serving.status_memory(7, "binding-a", "blake3-256:artifact-a");
    let repeated = serving.status_memory(7, "binding-a", "blake3-256:artifact-a");
    let refreshed = serving.status_memory(7, "binding-a", "blake3-256:artifact-b");

    assert_eq!(first, repeated);
    assert_ne!(first, refreshed);
    assert_eq!(first.parent(), Some(serving.root()));
}
