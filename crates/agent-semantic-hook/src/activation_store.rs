//! Activation loading and provider manifest defaults for `agent-semantic-hook`.

use crate::cache_paths::project_hook_cache_dir;
use crate::protocol_activation::{HookActivation, HookRuntime, parse_activation};
use crate::provider_manifest::{build_default_activation, provider_manifests};
use crate::runtime_profile::{default_runtime_profiles_path, write_runtime_profiles_for_runtime};
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
    match load_activation(activation_path) {
        Ok(runtime) => Ok(runtime),
        Err(load_error) if is_generated_activation_path(activation_path) => {
            eprintln!(
                "[agent-semantic-hook] syncing generated activation {}: {load_error}",
                activation_path.display()
            );
            sync_activation(project_root, activation_path).map_err(|sync_error| {
                format!(
                    "{load_error}; failed to sync generated activation {}: {sync_error}",
                    activation_path.display()
                )
            })
        }
        Err(error) => Err(error),
    }
}

fn sync_activation(project_root: &Path, activation_path: &Path) -> Result<HookRuntime, String> {
    let activation = build_default_activation(project_root)?;
    write_activation(activation_path, &activation)?;
    let runtime = activation_to_runtime(&activation)?;
    let runtime_profiles_path = default_runtime_profiles_path(project_root)?;
    write_runtime_profiles_for_runtime(&runtime_profiles_path, project_root, &runtime)?;
    Ok(runtime)
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
    project_hook_cache_dir(project_root)
        .unwrap_or_else(|_| {
            project_root
                .join(".cache")
                .join("agent-semantic-protocol")
                .join("hooks")
        })
        .join("activation.json")
}

/// Search ancestors for a managed hook activation cache file.
pub fn discover_activation_path(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(default_activation_path)
        .find(|path| path.is_file())
}

pub(crate) fn is_generated_activation_path(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.ends_with(".cache/agent-semantic-protocol/hooks/activation.json")
}
/// Parses a project hook activation using the built-in provider manifests.
pub fn parse_hook_activation(input: &str) -> Result<HookRuntime, crate::protocol::AgentHookError> {
    let manifests = provider_manifests();
    parse_activation(input, &manifests)
}
