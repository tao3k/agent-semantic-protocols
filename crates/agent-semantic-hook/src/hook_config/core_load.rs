use std::path::{Path, PathBuf};

use agent_semantic_config::{
    default_hook_client_config_template, default_hook_client_config_template_for_source_extensions,
    load_asp_project_config_file, load_hook_client_config_file,
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

/// Render the seed global hook config file for active provider source extensions.
pub fn default_client_config_template_for_source_extensions<I, S>(source_extensions: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    default_hook_client_config_template_for_source_extensions(source_extensions)
}

/// Load and compile hook config rules.
pub fn load_client_config(path: &Path) -> Result<ClientHookConfig, String> {
    let parsed = load_hook_client_config_file(path)?;
    compile_config(parsed)
}

/// Load optional user hook config and validate hook-owned project config fields.
pub fn load_client_config_for_project(
    path: &Path,
    project_root: &Path,
) -> Result<ClientHookConfig, String> {
    let parsed = if path.is_file() {
        load_hook_client_config_file(path)?
    } else {
        agent_semantic_config::default_hook_client_config_file()?
    };
    let agent_config_path = project_agent_config_path(project_root);
    load_asp_project_config_file(&agent_config_path)?;
    compile_config(parsed)
}
