//! Codex project-trust install helpers without hook event state.

use crate::codex_config::validate_codex_config_toml;
use crate::codex_trust::merge_codex_project_trust_config;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Install user-level Codex project trust without writing hook event trust state.
pub fn install_codex_user_project_trust(project_config_path: &Path) -> Result<PathBuf, String> {
    let project_config_path = fs::canonicalize(project_config_path).map_err(|error| {
        format!(
            "failed to resolve project Codex config {}: {error}",
            project_config_path.display()
        )
    })?;
    let project_root = project_root_for_codex_config_path(&project_config_path)?;
    let codex_home = codex_home_path()?;
    fs::create_dir_all(&codex_home)
        .map_err(|error| format!("failed to create {}: {error}", codex_home.display()))?;
    let user_config_path = codex_home.join("config.toml");
    let existing = fs::read_to_string(&user_config_path).unwrap_or_default();
    if user_config_path.is_file() {
        validate_codex_config_toml(&existing).map_err(|error| {
            format!(
                "refusing to write invalid Codex user config {}: {error}",
                user_config_path.display()
            )
        })?;
    }
    let merged = merge_codex_project_trust_config(&existing, &project_root)?;
    validate_codex_config_toml(&merged).map_err(|error| {
        format!(
            "refusing to write invalid Codex user project trust config {}: {error}",
            user_config_path.display()
        )
    })?;
    if merged != existing {
        fs::write(&user_config_path, merged.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", user_config_path.display()))?;
    }
    Ok(user_config_path)
}

fn codex_home_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".codex"))
        .ok_or_else(|| {
            "missing CODEX_HOME and HOME; cannot write Codex project trust state".to_string()
        })
}

fn project_root_for_codex_config_path(project_config_path: &Path) -> Result<PathBuf, String> {
    let codex_dir = project_config_path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", project_config_path.display()))?;
    if codex_dir.file_name().and_then(|name| name.to_str()) != Some(".codex") {
        return Err(format!(
            "{} is not a project .codex/config.toml path",
            project_config_path.display()
        ));
    }
    codex_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{} has no project root parent", codex_dir.display()))
}
