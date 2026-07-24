use std::path::{Path, PathBuf};

pub(super) fn for_workspace(root: &Path) -> PathBuf {
    agent_semantic_config::project_activation_path(root)
        .expect("resolve canonical project activation path")
}
