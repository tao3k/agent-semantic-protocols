//! CLI/install adapter over the provider-protocol registration contract.

use sha2::Digest;

pub(super) use agent_semantic_provider_protocol::ProviderInstallRegistration;

const REGISTER_JSON: &str = include_str!("../../../../../schemas/provider-install-register.json");

pub(crate) fn provider_install_registrations() -> Result<Vec<ProviderInstallRegistration>, String> {
    agent_semantic_provider_protocol::parse_provider_install_register(REGISTER_JSON.as_bytes())
        .map(|register| register.providers)
}

/// Resolve the canonical registration for a language provider.
pub(crate) fn provider_install_registration(
    language_id: &str,
) -> Result<ProviderInstallRegistration, String> {
    provider_install_registrations()?
        .into_iter()
        .find(|registration| registration.language_id == language_id)
        .ok_or_else(|| format!("no installable provider is registered for `{language_id}`"))
}

pub(crate) fn provider_install_registration_digest(
    registration: &ProviderInstallRegistration,
) -> Result<String, String> {
    let bytes = serde_json::to_vec(registration)
        .map_err(|error| format!("encode provider install registration: {error}"))?;
    Ok(format!("sha256:{:x}", sha2::Sha256::digest(bytes)))
}

pub(crate) fn provider_install_registry_digest() -> Result<String, String> {
    let register = agent_semantic_provider_protocol::parse_provider_install_register(
        REGISTER_JSON.as_bytes(),
    )?;
    let bytes = serde_json::to_vec(&register)
        .map_err(|error| format!("encode provider install register: {error}"))?;
    Ok(format!("sha256:{:x}", sha2::Sha256::digest(bytes)))
}
