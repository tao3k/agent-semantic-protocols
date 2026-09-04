//! State Home layout tests.

use crate::ProjectBinding;
use crate::StateHomeLayout;

#[test]
fn physical_namespace_has_no_schema_version_suffix() {
    let binding = ProjectBinding::resolve(None, "repo", "/tmp/workspace").unwrap();
    let layout = StateHomeLayout::new("/tmp/state-home");
    let workspace = layout.workspace(&binding.workspace).unwrap();

    for path in [
        &layout.catalog,
        &layout.workspaces,
        &layout.runtime,
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
