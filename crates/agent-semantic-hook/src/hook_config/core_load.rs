use std::path::{Path, PathBuf};

use agent_semantic_config::{
    default_hook_client_config_template, load_asp_project_config_file,
    load_hook_client_config_file, merge_asp_project_hook_config,
};

use crate::hook_config::core::{
    ClientHookConfig, compile_config, compile_config_with_executable_capabilities,
};
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

/// Return the identity of the fully rendered default Hook policy projection.
pub fn default_client_config_projection_digest() -> String {
    use sha2::{Digest, Sha256};

    format!(
        "sha256:{:x}",
        Sha256::digest(default_client_config_template().as_bytes())
    )
}

/// Return the identities that make a Hook executable compatible with this library.
pub fn hook_runtime_artifact_fingerprint() -> String {
    format!(
        "registry={};config={}",
        crate::semantic_registry_digest(),
        default_client_config_projection_digest()
    )
}

pub(crate) fn default_client_config_file()
-> Result<agent_semantic_config::HookClientConfigFile, String> {
    toml::from_str(&default_client_config_template())
        .map_err(|error| format!("failed to parse provider-projected hook config: {error}"))
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
    let parsed = agent_semantic_config::load_hook_client_config_file(path)?;
    let agent_config_path = project_agent_config_path(project_root);
    let project = load_asp_project_config_file(&agent_config_path)?;
    compile_config(merge_asp_project_hook_config(parsed, project)?)
}

/// Load user-owned Hook policy. Provider identity and routes are Runtime
/// Register state and are not copied into Hook configuration.
pub fn load_client_config_for_matcher_publication(
    path: &Path,
    project_root: &Path,
) -> Result<ClientHookConfig, String> {
    let parsed = agent_semantic_config::load_hook_client_config_file(path)?;
    let agent_config_path = project_agent_config_path(project_root);
    let project = load_asp_project_config_file(&agent_config_path)?;
    compile_config(merge_asp_project_hook_config(parsed, project)?)
}

/// Compile a policy against an explicit executable-capability snapshot.
///
/// This is the deterministic boundary used by black-box policy composition
/// tests. Production callers use `load_client_config_for_project`, which
/// captures the host snapshot once during compilation.
pub fn load_client_config_for_project_with_executable_capabilities<I, S>(
    path: &Path,
    project_root: &Path,
    capabilities: I,
) -> Result<ClientHookConfig, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let parsed = agent_semantic_config::load_hook_client_config_file(path)?;
    let agent_config_path = project_agent_config_path(project_root);
    let project = load_asp_project_config_file(&agent_config_path)?;
    let capabilities = capabilities
        .into_iter()
        .map(Into::into)
        .collect::<std::collections::BTreeSet<_>>();
    compile_config_with_executable_capabilities(
        merge_asp_project_hook_config(parsed, project)?,
        Some(&capabilities),
    )
}

/// Load a partial hook config over the embedded defaults, then apply
/// project-local hook declarations.
pub fn load_client_config_overlay_for_project(
    path: &Path,
    project_root: &Path,
) -> Result<ClientHookConfig, String> {
    let parsed = agent_semantic_config::load_hook_client_config_overlay_file(path)?;
    let agent_config_path = project_agent_config_path(project_root);
    let project = load_asp_project_config_file(&agent_config_path)?;
    compile_config(merge_asp_project_hook_config(parsed, project)?)
}

/// Compile the binary-owned managed template without depending on its disk cache.
pub fn load_embedded_client_config_for_project(
    project_root: &Path,
) -> Result<ClientHookConfig, String> {
    let parsed = default_client_config_file()?;
    let agent_config_path = project_agent_config_path(project_root);
    let project = load_asp_project_config_file(&agent_config_path)?;
    compile_config(merge_asp_project_hook_config(parsed, project)?)
}
