// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Read-only Codex plugin activation configuration.

use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Stable identifier used as a key in Codex's global plugin table.
pub struct CodexPluginId(String);

impl CodexPluginId {
    /// Returns the exact plugin-table key without changing its spelling.
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
/// Typed failure returned while decoding Codex's plugin configuration.
pub struct CodexPluginConfigError(String);

impl std::fmt::Display for CodexPluginConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CodexPluginConfigError {}

impl From<String> for CodexPluginConfigError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Reports whether `plugin_id` is explicitly enabled in a Codex config file.
///
/// A missing file, plugin table, plugin entry, or `enabled` field is treated as
/// disabled. Malformed TOML remains a typed error rather than being accepted.
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
