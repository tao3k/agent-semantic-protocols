//! Read-only Codex plugin activation configuration.

use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexPluginId(String);

impl CodexPluginId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for CodexPluginId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CodexPluginId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexPluginConfigError(String);

impl From<String> for CodexPluginConfigError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

pub fn codex_config_plugin_enabled(
    config_path: &Path,
    plugin_id: CodexPluginId,
) -> Result<bool, CodexPluginConfigError> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    let parsed = toml::from_str::<toml::Value>(&existing)
        .map_err(|error| format!("invalid Codex plugin config TOML: {error}"))?;
    Ok(parsed
        .get("plugins")
        .and_then(toml::Value::as_table)
        .and_then(|plugins| plugins.get(plugin_id.as_str()))
        .and_then(toml::Value::as_table)
        .and_then(|plugin| plugin.get("enabled"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false))
}
