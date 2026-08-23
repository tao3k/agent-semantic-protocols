//! Provider installation authority, independent from Hook and Runtime routes.

use serde::{Deserialize, Serialize};

const REGISTER_JSON: &str = include_str!("../../../../schemas/provider-install-register.json");

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ProviderInstallArtifactDomain {
    Checkout,
    StateHomeProviderStaging,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProviderInstallRegistration {
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) binary: String,
    pub(super) source_root: String,
    pub(super) workspace_install: String,
    pub(super) build_binding: String,
    pub(super) artifact_domain: ProviderInstallArtifactDomain,
}

#[cfg(test)]
impl ProviderInstallRegistration {
    pub(super) fn language_id(&self) -> &String {
        &self.language_id
    }

    pub(super) fn provider_id(&self) -> &String {
        &self.provider_id
    }

    pub(super) fn binary(&self) -> &str {
        &self.binary
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderInstallRegister {
    #[serde(rename = "$schema")]
    schema: String,
    schema_id: String,
    schema_version: String,
    providers: Vec<ProviderInstallRegistration>,
}

fn register() -> Result<ProviderInstallRegister, String> {
    let register: ProviderInstallRegister = serde_json::from_str(REGISTER_JSON)
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
                "provider install identity drift: languageId={} expectedProviderId={expected_provider}",
                registration.language_id
            ));
        }
    }
    Ok(register)
}

pub(super) fn provider_install_registration(
    language_id: &str,
) -> Result<ProviderInstallRegistration, String> {
    register()?
        .providers
        .into_iter()
        .find(|registration| registration.language_id == language_id)
        .ok_or_else(|| format!("no installable provider is registered for `{language_id}`"))
}

pub(super) fn provider_install_registrations() -> Result<Vec<ProviderInstallRegistration>, String> {
    register().map(|register| register.providers)
}

pub(super) fn provider_install_registration_digest(
    registration: &ProviderInstallRegistration,
) -> Result<String, String> {
    use sha2::Digest;
    let bytes = serde_json::to_vec(registration)
        .map_err(|error| format!("encode provider install registration: {error}"))?;
    Ok(format!("sha256:{:x}", sha2::Sha256::digest(bytes)))
}
