//! Activation loading and provider manifest defaults for `agent-semantic-hook`.

use crate::protocol_activation::{HookActivation, HookRuntime, parse_activation};
use crate::provider_manifest::{
    ProviderCommandSelection, build_default_activation, provider_command_selections,
    provider_manifests,
};
use agent_semantic_runtime::{
    discover_project_activation_path, is_project_activation_path, project_activation_path,
    project_local_activation_path,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Load and validate a project hook activation from `activation.json`.
pub fn load_activation(path: &Path) -> Result<HookRuntime, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read activation {}: {error}", path.display()))?;
    parse_activation(&contents, &provider_manifests())
        .map_err(|error| format!("invalid activation JSON: {error:?}"))
}

/// Load an activation or regenerate the managed cache copy when it has drifted.
pub fn load_or_sync_activation(
    activation_path: &Path,
    project_root: &Path,
) -> Result<HookRuntime, String> {
    if is_generated_activation_path(activation_path) {
        return sync_activation(project_root, activation_path).or_else(|sync_error| {
            load_activation(activation_path).map_err(|load_error| {
                format!(
                    "{load_error}; failed to sync generated activation {}: {sync_error}",
                    activation_path.display()
                )
            })
        });
    }
    load_activation(activation_path)
}

/// Result of syncing the generated default activation during install.
pub struct DefaultActivationSync {
    pub activation: HookActivation,
    pub status: &'static str,
}

/// Load the generated activation when provider command selection is unchanged,
/// otherwise rebuild it from the current project.
pub fn load_or_refresh_default_activation(
    activation_path: &Path,
    project_root: &Path,
) -> Result<DefaultActivationSync, String> {
    let current_selections = provider_command_selections(project_root)?;
    if let Some(activation) =
        reusable_activation(activation_path, project_root, &current_selections)?
    {
        return Ok(DefaultActivationSync {
            activation,
            status: "reused",
        });
    }

    let existed = activation_path.is_file();
    let activation = build_default_activation(project_root)?;
    write_activation(activation_path, &activation)?;
    Ok(DefaultActivationSync {
        activation,
        status: if existed { "refreshed" } else { "created" },
    })
}

fn reusable_activation(
    activation_path: &Path,
    project_root: &Path,
    current_selections: &[ProviderCommandSelection],
) -> Result<Option<HookActivation>, String> {
    let contents = match fs::read_to_string(activation_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read activation {}: {error}",
                activation_path.display()
            ));
        }
    };
    if parse_activation(&contents, &provider_manifests()).is_err() {
        return Ok(None);
    }
    let Ok(activation) = serde_json::from_str::<HookActivation>(&contents) else {
        return Ok(None);
    };
    if activation.project_root != project_root.display().to_string() {
        return Ok(None);
    }
    if activation_matches_provider_command_selections(&activation, current_selections)
        && activation_matches_current_manifest_coverage(&activation)
    {
        Ok(Some(activation))
    } else {
        Ok(None)
    }
}

fn activation_matches_provider_command_selections(
    activation: &HookActivation,
    current_selections: &[ProviderCommandSelection],
) -> bool {
    activation.providers.len() == current_selections.len()
        && activation
            .providers
            .iter()
            .zip(current_selections)
            .all(|(provider, selection)| {
                provider.manifest_id == selection.manifest_id
                    && provider.manifest_digest == selection.manifest_digest
                    && provider.language_id == selection.language_id
                    && provider.provider_id == selection.provider_id
                    && provider.binary == selection.binary
                    && provider.execution == selection.execution
                    && provider.provider_command_prefix == selection.provider_command_prefix
            })
}

fn activation_matches_current_manifest_coverage(activation: &HookActivation) -> bool {
    let manifests = provider_manifests();
    activation.providers.iter().all(|provider| {
        manifests
            .iter()
            .find(|manifest| manifest.manifest_id == provider.manifest_id)
            .is_some_and(|manifest| {
                provider.coverage.source_roots == manifest.source.default_source_roots
                    && provider.coverage.config_files == manifest.source.default_config_files
                    && provider.coverage.source_extensions == manifest.source.default_extensions
                    && provider.coverage.ignored_path_prefixes
                        == manifest.source.default_ignored_path_prefixes
            })
    })
}

fn sync_activation(project_root: &Path, activation_path: &Path) -> Result<HookRuntime, String> {
    let sync = load_or_refresh_default_activation(activation_path, project_root)?;
    activation_to_runtime(&sync.activation)
}

fn activation_to_runtime(activation: &HookActivation) -> Result<HookRuntime, String> {
    let contents = serde_json::to_string(activation)
        .map_err(|error| format!("failed to serialize generated activation: {error}"))?;
    parse_activation(&contents, &provider_manifests()).map_err(|error| format!("{error:?}"))
}

/// Write a pretty JSON project hook activation.
pub fn write_activation(path: &Path, activation: &HookActivation) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let output = serde_json::to_string_pretty(activation)
        .map_err(|error| format!("failed to serialize activation: {error}"))?;
    fs::write(path, format!("{}\n", output.trim_end()))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

/// Return the managed cache path for a project's hook activation.
pub fn default_activation_path(project_root: &Path) -> PathBuf {
    project_activation_path(project_root)
        .unwrap_or_else(|_| project_local_activation_path(project_root))
}

/// Search ancestors for a managed hook activation cache file.
pub fn discover_activation_path(start: &Path) -> Option<PathBuf> {
    discover_project_activation_path(start)
}

pub(crate) fn is_generated_activation_path(path: &Path) -> bool {
    is_project_activation_path(path)
}
/// Parses a project hook activation using the built-in provider manifests.
pub fn parse_hook_activation(input: &str) -> Result<HookRuntime, crate::protocol::AgentHookError> {
    let manifests = provider_manifests();
    parse_activation(input, &manifests)
}
