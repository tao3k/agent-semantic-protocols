//! Typed manifest for Provider installation inputs.

use serde::Deserialize;
use serde::Serialize;

/// Filesystem authority that owns a Provider installation artifact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderInstallArtifactDomain {
    Checkout,
    StateHomeProviderStaging,
}

/// One Provider installation registration.
///
/// This is an intentional raw DTO boundary matching the canonical JSON schema.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInstallRegistration {
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    pub source_root: String,
    pub workspace_install: String,
    pub build_binding: String,
    pub artifact_domain: ProviderInstallArtifactDomain,
}

/// Canonical set of Provider installation registrations.
///
/// This is an intentional raw DTO boundary matching the canonical JSON schema.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInstallRegister {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub schema_id: String,
    pub schema_version: String,
    pub providers: Vec<ProviderInstallRegistration>,
}

/// Decode and validate a canonical Provider installation register.
pub fn parse_provider_install_register(bytes: &[u8]) -> Result<ProviderInstallRegister, String> {
    let register: ProviderInstallRegister = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode provider install register: {error}"))?;
    if register.schema.is_empty()
        || register.schema_id != "agent.semantic-protocols.provider-install-register"
        || register.schema_version != "1"
    {
        return Err("provider install register schema identity mismatch".to_owned());
    }
    let mut languages = std::collections::BTreeSet::new();
    let mut providers = std::collections::BTreeSet::new();
    for registration in &register.providers {
        if !languages.insert(registration.language_id.as_str())
            || !providers.insert(registration.provider_id.as_str())
        {
            return Err("provider install register identities must be unique".to_owned());
        }
        let expected_provider = format!("asp-{}", registration.language_id);
        if expected_provider != registration.provider_id || registration.binary != expected_provider
        {
            return Err(format!(
                "provider install identity drift: languageId={}",
                registration.language_id
            ));
        }
    }
    Ok(register)
}
