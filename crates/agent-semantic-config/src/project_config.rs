// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Parses repository-scoped ASP project configuration (`asp.toml`).

use std::path::Path;

use serde::Deserialize;

/// Parsed ASP project config from `asp.toml` or `.agents/asp.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AspProjectConfigFile {
    #[serde(default)]
    pub discovery: AspProjectDiscoveryConfig,
}

/// ASP-owned repository candidate policy from `[discovery]`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspProjectDiscoveryConfig {
    #[serde(default)]
    pub ignored_dir_names: Option<Vec<String>>,
    #[serde(default)]
    pub include_hidden_dir_names: Option<Vec<String>>,
}

/// Load the ASP project config. Unknown project sections are ignored here; each
/// subsystem owns its own parsed subset.
pub fn load_asp_project_config_file(path: &Path) -> Result<AspProjectConfigFile, String> {
    if !path.is_file() {
        return Ok(AspProjectConfigFile::default());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str::<AspProjectConfigFile>(&content)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}
