use crate::provider_registry::provider_register;
use crate::provider_registry::registered_language_ids;
use crate::provider_registry::registered_provider_id;
use crate::provider_registry::semantic_registry_digest;

#[test]
fn provider_register_has_unique_language_and_provider_identities() {
    let register = provider_register();
    assert!(!register.providers.is_empty());

    let mut language_ids = std::collections::BTreeSet::new();
    let mut provider_ids = std::collections::BTreeSet::new();
    for provider in &register.providers {
        assert!(language_ids.insert(provider.language_id.as_str()));
        assert!(provider_ids.insert(provider.provider_id.as_str()));
    }
}

#[test]
fn embedded_provider_register_preserves_authoritative_descriptors() {
    let register: serde_json::Value =
        serde_json::from_str(agent_semantic_provider_protocol::builtin_provider_register_json())
            .expect("embedded provider register JSON");
    let providers = register["providers"].as_array().expect("providers");
    assert!(!providers.is_empty());
    for provider in providers {
        let provider = provider.as_object().expect("provider descriptor object");
        for required in [
            "languageId",
            "providerId",
            "execution",
            "providerDescriptor",
            "queryPackDescriptor",
            "routes",
            "runtimeContract",
            "schemas",
            "searchCapabilities",
            "sourceInventory",
        ] {
            assert!(
                provider.contains_key(required),
                "provider registration is missing `{required}`"
            );
        }
    }
}

#[test]
fn provider_identity_lookup_matches_registered_languages() {
    for language_id in registered_language_ids() {
        let provider_id = registered_provider_id(language_id.as_str())
            .unwrap_or_else(|| panic!("missing provider identity for {language_id:?}"));
        assert!(provider_id.starts_with("asp-"));
    }
}

#[test]
fn identity_catalog_lookup_is_warm_and_sub_millisecond() {
    let languages = registered_language_ids();
    for language_id in &languages {
        assert!(registered_provider_id(language_id.as_str()).is_some());
    }

    let mut samples = Vec::with_capacity(128);
    for _ in 0..128 {
        let started = std::time::Instant::now();
        for language_id in &languages {
            std::hint::black_box(registered_provider_id(language_id.as_str()));
        }
        std::hint::black_box(semantic_registry_digest());
        samples.push(started.elapsed());
    }

    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99).div_ceil(100) - 1];
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "warm identity catalog lookup p99 exceeded one millisecond: {p99:?}"
    );
}
