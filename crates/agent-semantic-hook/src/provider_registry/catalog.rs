//! Registry-style language registrations used to derive hook provider manifests.

use serde::Deserialize;

use crate::protocol_activation::protocol_activation_manifest::ProviderManifest;

fn provider_register_json() -> &'static str {
    agent_semantic_provider_protocol::builtin_provider_register_json()
}

pub fn semantic_registry_digest() -> String {
    static DIGEST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIGEST
        .get_or_init(|| {
            let digest =
                <sha2::Sha256 as sha2::Digest>::digest(provider_register_json().as_bytes());
            format!("sha256:{digest:x}")
        })
        .clone()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDevelopmentRegistration {
    pub language_id: agent_semantic_config::LanguageId,
    pub provider_id: agent_semantic_config::ProviderId,
    pub binary: String,
    pub development:
        crate::protocol_activation::protocol_activation_manifest::ProviderDevelopmentDescriptor,
}

pub fn registered_provider_projection_operation(
    language_id: &str,
    provider_id: &str,
) -> Result<Option<String>, String> {
    let Some(registration) = provider_register()
        .providers
        .iter()
        .find(|registration| registration.language_id == language_id)
    else {
        return Ok(None);
    };
    if registration.provider_id != provider_id {
        return Err(format!(
            "ProviderRegistry provider drift for language `{language_id}`: expected {}, got {provider_id}",
            registration.provider_id
        ));
    }
    Ok(None)
}

pub fn registered_provider_method_invocation(
    language_id: &str,
    provider_id: &str,
    method: &str,
) -> Result<Option<crate::protocol::CommandTemplate>, String> {
    let register = provider_register();
    let Some(language) = register
        .providers
        .iter()
        .find(|language| language.language_id == language_id)
    else {
        return Ok(None);
    };
    if language.provider_id != provider_id {
        return Err(format!(
            "ProviderRegistry provider drift for language `{language_id}`: expected {}, got {provider_id}",
            language.provider_id
        ));
    }
    let _ = method;
    Ok(None)
}

pub fn registered_provider_id(language_id: &str) -> Option<String> {
    provider_register()
        .providers
        .iter()
        .find(|language| language.language_id == language_id)
        .map(|language| language.provider_id.clone())
}

/// Registry-owned execution class derived from the provider manifest contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisteredProviderKind {
    ProgrammingLanguage,
    Document,
}

/// Return the execution class declared by the registered provider manifest.
///
/// Programming-language providers own package/project resolution and may be
/// installed independently. Document providers are compiled into the ASP
/// runtime and must never enter the external language-provider installer.
pub fn registered_provider_kind(language_id: &str) -> Result<RegisteredProviderKind, String> {
    let registration = provider_register()
        .providers
        .iter()
        .find(|registration| registration.language_id == language_id)
        .ok_or_else(|| format!("no registered provider for language `{language_id}`"))?;
    Err(format!(
        "provider `{}` requires an activated runtime descriptor",
        registration.provider_id
    ))
}

pub fn materialize_provider_routes(
    manifest: &ProviderManifest,
) -> Result<crate::protocol::HookRoutes, String> {
    Err(format!(
        "provider `{}` language `{}` requires activated runtime routes",
        manifest.provider_id, manifest.language_id
    ))
}

/// Return registered ASP language ids from the provider registry schema.
pub fn registered_language_ids() -> Vec<agent_semantic_config::LanguageId> {
    provider_register()
        .providers
        .iter()
        .map(|registration| {
            agent_semantic_config::LanguageId::new(registration.language_id.as_str())
        })
        .collect()
}

pub(crate) fn provider_register() -> &'static ProviderRegister {
    static REGISTER: std::sync::OnceLock<ProviderRegister> = std::sync::OnceLock::new();
    REGISTER.get_or_init(|| {
        let register: ProviderRegister = serde_json::from_str(provider_register_json())
            .expect("embedded provider register must be valid JSON");
        validate_provider_register(&register)
            .expect("embedded provider register must be internally consistent");
        register
    })
}

fn validate_provider_register(register: &ProviderRegister) -> Result<(), String> {
    let mut language_ids = std::collections::BTreeSet::new();
    let mut provider_ids = std::collections::BTreeSet::new();
    for language in &register.providers {
        if language.language_id.is_empty() || language.provider_id.is_empty() {
            return Err("ProviderRegistry identities must be non-empty".to_string());
        }
        if !language_ids.insert(language.language_id.as_str()) {
            return Err(format!(
                "duplicate ProviderRegistry languageId `{}`",
                language.language_id
            ));
        }
        if !provider_ids.insert(language.provider_id.as_str()) {
            return Err(format!(
                "duplicate ProviderRegistry providerId `{}`",
                language.provider_id
            ));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderRegister {
    pub(crate) providers: Vec<ProviderRegistration>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderRegistration {
    pub(crate) language_id: String,
    pub(crate) provider_id: String,
}
