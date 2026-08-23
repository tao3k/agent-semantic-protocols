use crate::provider_registry::{
    provider_register, registered_language_ids, registered_provider_id, semantic_registry_digest,
};

#[test]
fn provider_register_is_identity_only_and_unique() {
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
fn embedded_provider_register_has_no_execution_descriptors() {
    let register: serde_json::Value =
        serde_json::from_str(agent_semantic_provider_protocol::builtin_provider_register_json())
            .expect("embedded provider register JSON");
    let providers = register["providers"].as_array().expect("providers");
    assert!(!providers.is_empty());
    for provider in providers {
        let fields = provider
            .as_object()
            .expect("provider identity object")
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            fields,
            std::collections::BTreeSet::from(["languageId", "providerId"])
        );
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
    let started = std::time::Instant::now();
    for _ in 0..128 {
        for language_id in &languages {
            std::hint::black_box(registered_provider_id(language_id.as_str()));
        }
        std::hint::black_box(semantic_registry_digest());
    }
    assert!(
        started.elapsed() < std::time::Duration::from_millis(1),
        "warm identity catalog lookup exceeded one millisecond: {:?}",
        started.elapsed()
    );
}
