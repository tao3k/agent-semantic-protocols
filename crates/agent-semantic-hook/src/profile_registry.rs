//! Retired provider profile cache cleanup for `agent-semantic-protocol` hooks.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Remove retired hook cache files from previous `semantic-agent` layouts.
pub fn remove_retired_codex_hook_cache_files(project_root: &Path) -> Result<(), String> {
    remove_profile_cache_files(
        &project_root
            .join(".cache")
            .join("agent-semantic-protocol")
            .join("hooks"),
    )?;
    let retired_codex_dir = project_root.join(".codex").join("agent-semantic-hook");
    remove_retired_hook_dir(&retired_codex_dir)?;
    let retired_default_cache_dir = project_root.join(".cache").join("agent-semantic-hook");
    remove_retired_hook_dir(&retired_default_cache_dir)?;
    let retired_semantic_protocol_cache_dir =
        project_root.join(".cache").join("semantic-agent-protocol");
    remove_retired_hook_dir(&retired_semantic_protocol_cache_dir.join("hooks"))?;
    remove_empty_dir(&retired_semantic_protocol_cache_dir)?;
    if let Some(cache_root) = env::var_os("PRJ_CACHE_HOME").filter(|value| !value.is_empty()) {
        let cache_root = PathBuf::from(cache_root);
        remove_profile_cache_files(&cache_root.join("agent-semantic-protocol").join("hooks"))?;
        remove_retired_hook_dir(&cache_root.join("agent-semantic-hook"))?;
        let retired_semantic_protocol_cache_dir = cache_root.join("semantic-agent-protocol");
        remove_retired_hook_dir(&retired_semantic_protocol_cache_dir.join("hooks"))?;
        remove_empty_dir(&retired_semantic_protocol_cache_dir)?;
    }
    Ok(())
}

fn remove_profile_cache_files(profiles_dir: &Path) -> Result<(), String> {
    let path = profiles_dir.join("profiles.json");
    if path.is_file() {
        fs::remove_file(&path)
            .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
    }
    remove_retired_profile_shards(profiles_dir)
}

fn remove_retired_hook_dir(retired_dir: &Path) -> Result<(), String> {
    if !retired_dir.is_dir() {
        return Ok(());
    }
    fs::remove_dir_all(retired_dir)
        .map_err(|error| format!("failed to remove {}: {error}", retired_dir.display()))
}

fn remove_empty_dir(path: &Path) -> Result<(), String> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(error) => Err(format!("failed to remove {}: {error}", path.display())),
    }
}

fn remove_retired_profile_shards(profiles_dir: &Path) -> Result<(), String> {
    if !profiles_dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(profiles_dir)
        .map_err(|error| format!("failed to read {}: {error}", profiles_dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read profile registry entry: {error}"))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name != "profiles.json"
            && file_name.starts_with("profiles.")
            && file_name.ends_with(".json")
        {
            fs::remove_file(entry.path())
                .map_err(|error| format!("failed to remove {}: {error}", entry.path().display()))?;
        }
    }
    Ok(())
}
