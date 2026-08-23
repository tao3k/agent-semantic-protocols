use std::path::{Path, PathBuf};

pub fn project_agent_config_path(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join("asp.toml")
}
