// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical provider-register composition for build scripts.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Build output containing the composed provider register and all tracked inputs.
pub struct ResolvedProviderRegisterBuild {
    pub bytes: Vec<u8>,
    pub input_paths: Vec<PathBuf>,
    pub identities: Vec<ProviderIdentityBuild>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Canonical language and provider identity discovered from the provider register.
pub struct ProviderIdentityBuild {
    pub language_id: String,
    pub provider_id: String,
}

/// Resolves canonical provider registrations and their build-script rerun inputs.
pub fn resolve_provider_register(
    source_root: impl AsRef<Path>,
) -> Result<ResolvedProviderRegisterBuild, String> {
    let root = source_root.as_ref();
    let index = root.join("schemas/provider-register.json");
    let install_index = root.join("schemas/provider-install-register.json");
    let mut register = read_json(&index)?;
    let install_register = read_json(&install_index)?;
    let providers = register["providers"]
        .as_array()
        .ok_or("provider register providers must be an array")?;
    let identities = providers
        .iter()
        .map(|entry| {
            Ok(ProviderIdentityBuild {
                provider_id: entry["providerId"]
                    .as_str()
                    .ok_or("provider register providerId missing")?
                    .to_owned(),
                language_id: entry["languageId"]
                    .as_str()
                    .ok_or("provider register languageId missing")?
                    .to_owned(),
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let install_providers = install_register["providers"]
        .as_array()
        .ok_or("provider install register providers must be an array")?;
    let mut registrations = Vec::with_capacity(identities.len());
    let mut input_paths = vec![index, install_index];
    for identity in &identities {
        let install = install_providers
            .iter()
            .find(|entry| {
                entry["languageId"].as_str() == Some(identity.language_id.as_str())
                    && entry["providerId"].as_str() == Some(identity.provider_id.as_str())
            })
            .ok_or_else(|| {
                format!(
                    "provider install register is missing {}/{}",
                    identity.language_id, identity.provider_id
                )
            })?;
        let source_root = install["sourceRoot"]
            .as_str()
            .ok_or("provider install register sourceRoot missing")?;
        let workspace_install = install["workspaceInstall"]
            .as_str()
            .ok_or("provider install register workspaceInstall missing")?;
        let workspace_install_path = root.join(source_root).join(workspace_install);
        let workspace = read_json(&workspace_install_path)?;
        let registration_reference = workspace["providerRegistration"]
            .as_str()
            .ok_or("provider workspace install providerRegistration missing")?;
        let registration_path = workspace_install_path
            .parent()
            .ok_or("provider workspace install parent missing")?
            .join(registration_reference);
        let registration = read_json(&registration_path)?;
        if registration["languageId"].as_str() != Some(identity.language_id.as_str())
            || registration["providerId"].as_str() != Some(identity.provider_id.as_str())
        {
            return Err(format!(
                "provider registration identity mismatch for {}/{}: {}",
                identity.language_id,
                identity.provider_id,
                registration_path.display()
            ));
        }
        input_paths.extend([workspace_install_path, registration_path]);
        registrations.push(registration);
    }
    register["providers"] = Value::Array(registrations);
    Ok(ResolvedProviderRegisterBuild {
        bytes: serde_json::to_vec_pretty(&register).map_err(|error| error.to_string())?,
        input_paths,
        identities,
    })
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_str(
        &fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{}: {error}", path.display()))
}
