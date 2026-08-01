use std::path::{Path, PathBuf};

use agent_semantic_config::{
    default_hook_client_config_template, load_asp_project_config_file,
    load_hook_client_config_file, load_hook_client_config_overlay_file,
    merge_asp_project_hook_config,
};

use crate::hook_config::core::{ClientHookConfig, compile_config};
use crate::hook_config_global::default_global_client_config_path;
use crate::provider_manifest::project_agent_config_path;

const REGISTERED_LANGUAGE_PROVIDERS_MARKER: &str = "# @REGISTERED_LANGUAGE_PROVIDERS@";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedLanguageProviderProjection {
    language_providers: Vec<agent_semantic_config::HookClientLanguageProviderConfig>,
}

/// Return the default global hook config path.
pub fn default_client_config_path(_project_root: &str) -> PathBuf {
    default_global_client_config_path()
        .unwrap_or_else(|| PathBuf::from(".agent-semantic-protocols/hooks/config.toml"))
}

/// Render the seed global hook config file.
pub fn default_client_config_template() -> String {
    let projection = managed_language_provider_projection()
        .expect("embedded provider manifests must render a valid hook config projection");
    default_hook_client_config_template()
        .replace(REGISTERED_LANGUAGE_PROVIDERS_MARKER, projection.trim_end())
}

pub(crate) fn default_client_config_file()
-> Result<agent_semantic_config::HookClientConfigFile, String> {
    toml::from_str(&default_client_config_template())
        .map_err(|error| format!("failed to parse provider-projected hook config: {error}"))
}

fn managed_language_provider_projection() -> Result<String, String> {
    let mut language_providers = crate::provider_manifest::builtin_provider_manifests()
        .into_iter()
        .map(|manifest| {
            let source_extensions = manifest
                .project_resolution()
                .map(|descriptor| descriptor.source_extensions.clone())
                .or_else(|| {
                    manifest
                        .document_resolution()
                        .map(|descriptor| descriptor.extensions.clone())
                })
                .ok_or_else(|| {
                    format!(
                        "provider `{}` omitted source extension authority",
                        manifest.provider_id()
                    )
                })?;
            Ok(agent_semantic_config::HookClientLanguageProviderConfig {
                language_id: manifest.language_id().as_str().to_owned(),
                provider_id: manifest.provider_id().as_str().to_owned(),
                manifest_digest: crate::provider_manifest_digest(&manifest)
                    .map_err(|error| error.to_string())?,
                source_extensions,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    language_providers.sort_by(|left, right| {
        (&left.language_id, &left.provider_id).cmp(&(&right.language_id, &right.provider_id))
    });
    toml::to_string(&ManagedLanguageProviderProjection { language_providers })
        .map_err(|error| format!("failed to serialize language provider projection: {error}"))
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

/// Load a partial hook config over the embedded defaults, then apply
/// project-local hook declarations.
pub fn load_client_config_overlay_for_project(
    path: &Path,
    project_root: &Path,
) -> Result<ClientHookConfig, String> {
    let parsed = load_hook_client_config_overlay_file(path)?;
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
