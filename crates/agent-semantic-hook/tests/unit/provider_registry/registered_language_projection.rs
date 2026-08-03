use super::{REGISTERED_LANGUAGE_ID_STRINGS, schema_registry};

#[test]
fn build_projection_matches_the_registry_schema() {
    let mut schema_language_ids = schema_registry()
        .languages
        .iter()
        .map(|registration| registration.language_id.as_str())
        .collect::<Vec<_>>();
    schema_language_ids.sort_unstable();
    schema_language_ids.dedup();
    assert_eq!(schema_language_ids, REGISTERED_LANGUAGE_ID_STRINGS);
}

#[test]
fn registered_provider_kind_owns_projection_capability_without_language_lists() {
    let manifests = crate::schema_registry_provider_manifests();
    crate::provider_registry::validate_registered_provider_projection_contracts(&manifests)
        .expect("registered providers share one capability-owned projection contract");
    let programming_provider_count = manifests
        .iter()
        .filter(|manifest| manifest.project_resolution().is_some())
        .count();
    let document_provider_count = manifests
        .iter()
        .filter(|manifest| manifest.document_resolution().is_some())
        .count();
    assert!(programming_provider_count > 0);
    assert!(document_provider_count > 0);
    assert_eq!(
        crate::registered_provider_kind("rust").expect("registered Rust provider kind"),
        crate::RegisteredProviderKind::ProgrammingLanguage,
    );
    assert_eq!(
        crate::registered_provider_kind("org").expect("registered Org provider kind"),
        crate::RegisteredProviderKind::Document,
    );
    assert_eq!(
        crate::registered_provider_kind("md").expect("registered Markdown provider kind"),
        crate::RegisteredProviderKind::Document,
    );
}
