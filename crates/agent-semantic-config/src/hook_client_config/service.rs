// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Validated service boundary over Hook configuration documents.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Path;

use super::document::AspProjectConfigFile;
use super::document::HookClientConfigFile;
use super::document::admit_canonical_provider_routes;
use super::document::parse_hook_client_config_file;
use super::document::parse_hook_client_config_overlay_file;
use super::validation::validate_config;

impl HookClientConfigFile {
    /// Validates the complete declarative Hook configuration contract.
    pub fn validate(&self) -> Result<(), String> {
        validate_config(self)
    }
}

/// Merges project-owned Hook declarations into a validated global base config.
///
/// Rule identifiers remain unique and project declarations replace only the
/// matching declarative rule rather than mutating compiled policy state.
pub fn merge_asp_project_hook_config(
    mut base: HookClientConfigFile,
    project: AspProjectConfigFile,
) -> Result<HookClientConfigFile, String> {
    validate_unique_project_rule_ids(&project)?;
    let mut base_rule_indices = base
        .rules
        .iter()
        .enumerate()
        .map(|(index, rule)| (rule.id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    for rule in project.hook.rules {
        if let Some(index) = base_rule_indices.get(&rule.id).copied() {
            base.rules[index] = rule;
        } else {
            base_rule_indices.insert(rule.id.clone(), base.rules.len());
            base.rules.push(rule);
        }
    }
    validate_config(&base)?;
    Ok(base)
}

fn validate_unique_project_rule_ids(project: &AspProjectConfigFile) -> Result<(), String> {
    let mut rule_ids = HashSet::new();
    for rule in &project.hook.rules {
        if !rule_ids.insert(rule.id.as_str()) {
            return Err(format!(
                "project hook declares rule `{}` more than once",
                rule.id
            ));
        }
    }
    Ok(())
}

/// Loads, parses, and validates a complete managed Hook config.
pub fn load_hook_client_config_file(path: &Path) -> Result<HookClientConfigFile, String> {
    let mut parsed = parse_hook_client_config_file(path)?;
    admit_canonical_provider_routes(&mut parsed)?;
    validate_config(&parsed)?;
    Ok(parsed)
}

/// Loads and validates an explicit Hook config overlay on the embedded defaults.
pub fn load_hook_client_config_overlay_file(path: &Path) -> Result<HookClientConfigFile, String> {
    let parsed = parse_hook_client_config_overlay_file(path)?;
    validate_config(&parsed)?;
    Ok(parsed)
}
