use std::path::{Path, PathBuf};

pub(super) fn for_workspace(root: &Path) -> PathBuf {
    agent_semantic_client_core::state_core::ResolvedState::resolve_with_state_home(
        root,
        root.join(".agent-semantic-protocols"),
    )
    .expect("resolve canonical project state")
    .paths
    .hooks_dir
    .join("state/activation.json")
}
