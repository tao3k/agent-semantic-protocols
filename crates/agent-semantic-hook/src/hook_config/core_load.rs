use std::path::{Path, PathBuf};

use agent_semantic_config::{
    default_hook_client_config_template, load_asp_project_config_file,
    load_hook_client_config_file, merge_asp_project_hook_config,
};

use crate::hook_config::core::{ClientHookConfig, compile_config};
use crate::hook_config_global::default_global_client_config_path;
use crate::provider_manifest::project_agent_config_path;

/// Return the default global hook config path.
pub fn default_client_config_path(_project_root: &str) -> PathBuf {
    default_global_client_config_path()
        .unwrap_or_else(|| PathBuf::from(".agent-semantic-protocols/hooks/config.toml"))
}

/// Render the seed global hook config file.
pub fn default_client_config_template() -> String {
    default_hook_client_config_template()
}

/// Load and compile hook config rules.
pub fn load_client_config(path: &Path) -> Result<ClientHookConfig, String> {
    let parsed = load_hook_client_config_file(path)?;
    compile_config(parsed)
}

/// Load the installed hook matcher config and validate hook-owned project fields.
pub fn load_client_config_for_project(
    path: &Path,
    project_root: &Path,
) -> Result<ClientHookConfig, String> {
    let parsed = load_hook_client_config_file(path)?;
    let agent_config_path = project_agent_config_path(project_root);
    let project = load_asp_project_config_file(&agent_config_path)?;
    compile_config(merge_asp_project_hook_config(parsed, project)?)
}

/// Compile the binary-owned managed template without depending on its disk cache.
pub fn load_embedded_client_config_for_project(
    project_root: &Path,
) -> Result<ClientHookConfig, String> {
    let parsed = agent_semantic_config::default_hook_client_config_file()?;
    let agent_config_path = project_agent_config_path(project_root);
    let project = load_asp_project_config_file(&agent_config_path)?;
    compile_config(merge_asp_project_hook_config(parsed, project)?)
}
