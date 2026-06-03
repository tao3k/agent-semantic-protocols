//! Activation loading and provider manifest defaults for `semantic-agent-hook`.

use crate::protocol_activation::{HookActivation, HookRuntime, parse_activation};
use crate::provider_manifest::{build_default_activation, provider_manifests};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn load_activation(path: &Path) -> Result<HookRuntime, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read activation {}: {error}", path.display()))?;
    parse_activation(&contents, &provider_manifests())
        .map_err(|error| format!("invalid activation JSON: {error:?}"))
}

pub(crate) fn load_or_sync_activation(
    activation_path: &Path,
    project_root: &Path,
) -> Result<HookRuntime, String> {
    match load_activation(activation_path) {
        Ok(runtime) => Ok(runtime),
        Err(load_error) if is_generated_activation_path(activation_path) => {
            eprintln!(
                "[semantic-agent-hook] syncing generated activation {}: {load_error}",
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
    activation_to_runtime(&activation)
}

fn activation_to_runtime(activation: &HookActivation) -> Result<HookRuntime, String> {
    let contents = serde_json::to_string(activation)
        .map_err(|error| format!("failed to serialize generated activation: {error}"))?;
    parse_activation(&contents, &provider_manifests()).map_err(|error| format!("{error:?}"))
}

pub(crate) fn write_activation(path: &Path, activation: &HookActivation) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let output = serde_json::to_string_pretty(activation)
        .map_err(|error| format!("failed to serialize activation: {error}"))?;
    fs::write(path, format!("{}\n", output.trim_end()))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(crate) fn default_activation_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".codex")
        .join("semantic-agent-hook")
        .join("activation.json")
}

pub(crate) fn discover_activation_path(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(default_activation_path)
        .find(|path| path.is_file())
}

pub(crate) fn is_generated_activation_path(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.ends_with(".codex/semantic-agent-hook/activation.json")
}
/// Parses a project hook activation using the built-in provider manifests.
pub fn parse_hook_activation(input: &str) -> Result<HookRuntime, crate::protocol::AgentHookError> {
    let manifests = provider_manifests();
    parse_activation(input, &manifests)
}
