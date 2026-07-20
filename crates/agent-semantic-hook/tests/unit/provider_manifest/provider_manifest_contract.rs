use agent_semantic_hook::{builtin_provider_manifests, validate_provider_manifest_contract};

fn canonical_manifest() -> agent_semantic_hook::ProviderManifest {
    builtin_provider_manifests()
        .into_iter()
        .next()
        .expect("builtin provider registry must not be empty")
}

#[test]
fn builtin_manifest_requires_source_snapshot_descriptor() {
    let mut manifest = canonical_manifest();
    manifest.search_capabilities.source_snapshot = None;

    assert_eq!(
        validate_provider_manifest_contract(&manifest),
        vec![format!(
            "provider `{}` is missing required searchCapabilities.sourceSnapshot descriptor",
            manifest.language_id
        )]
    );
}

#[test]
fn builtin_manifest_rejects_invalid_query_pack_descriptor_version() {
    let mut manifest = canonical_manifest();
    manifest.query_pack_descriptor.descriptor_version = "invalid".to_string();

    assert_eq!(
        validate_provider_manifest_contract(&manifest),
        vec![format!(
            "invalid-activation-config: provider manifest {} has an invalid queryPackDescriptor: identity, version, language, or recipes",
            manifest.manifest_id
        )]
    );
}
