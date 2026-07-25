use agent_semantic_hook::{builtin_provider_manifests, validate_provider_manifest_contract};

fn canonical_manifest() -> agent_semantic_hook::ProviderManifest {
    builtin_provider_manifests()
        .into_iter()
        .next()
        .expect("builtin provider registry must not be empty")
}

#[test]
fn builtin_manifest_requires_source_snapshot_descriptor() {
    let mut value =
        serde_json::to_value(canonical_manifest()).expect("serialize canonical manifest");
    value["searchCapabilities"]["sourceSnapshot"] = serde_json::Value::Null;
    let manifest =
        serde_json::from_value(value).expect("deserialize invalid source snapshot fixture");

    assert_eq!(
        validate_provider_manifest_contract(&manifest),
        vec![format!(
            "provider `{}` is missing required searchCapabilities.sourceSnapshot descriptor",
            manifest.language_id()
        )]
    );
}

#[test]
fn builtin_manifest_rejects_invalid_query_pack_descriptor_version() {
    let mut value =
        serde_json::to_value(canonical_manifest()).expect("serialize canonical manifest");
    value["queryPackDescriptor"]["descriptorVersion"] =
        serde_json::Value::String("invalid".to_string());
    let manifest = serde_json::from_value(value).expect("deserialize invalid query pack fixture");

    assert_eq!(
        validate_provider_manifest_contract(&manifest),
        vec![format!(
            "invalid-activation-config: provider manifest {} has an invalid queryPackDescriptor: identity, version, language, or recipes",
            manifest.manifest_id()
        )]
    );
}
