//! Read-only Codex plugin activation configuration.

use std::fs;
use std::path::Path;

pub fn codex_config_plugin_enabled(config_path: &Path, plugin_id: &str) -> Result<bool, String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    let parsed = toml::from_str::<toml::Value>(&existing)
        .map_err(|error| format!("invalid Codex plugin config TOML: {error}"))?;
    Ok(parsed
        .get("plugins")
        .and_then(toml::Value::as_table)
        .and_then(|plugins| plugins.get(plugin_id))
        .and_then(toml::Value::as_table)
        .and_then(|plugin| plugin.get("enabled"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false))
}
