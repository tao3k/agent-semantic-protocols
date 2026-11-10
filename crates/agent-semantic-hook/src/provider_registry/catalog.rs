// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Registry-style language registrations used to derive hook provider manifests.

use serde::Deserialize;

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

pub fn registered_provider_id(language_id: &str) -> Option<String> {
    provider_register()
        .providers
        .iter()
        .find(|language| language.language_id == language_id)
        .map(|language| language.provider_id.clone())
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
