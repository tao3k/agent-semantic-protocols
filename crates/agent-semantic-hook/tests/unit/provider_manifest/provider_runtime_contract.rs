use agent_semantic_hook::{builtin_provider_manifests, validate_provider_manifest_contract};

fn canonical_manifest_value() -> serde_json::Value {
    serde_json::to_value(
        builtin_provider_manifests()
            .into_iter()
            .next()
            .expect("builtin provider registry must not be empty"),
    )
    .expect("serialize canonical provider manifest")
}

#[test]
fn provider_manifest_requires_a_runtime_contract() {
    let mut value = canonical_manifest_value();
    value
        .as_object_mut()
        .expect("provider manifest object")
        .remove("runtimeContract");
    let error = serde_json::from_value::<agent_semantic_hook::ProviderManifest>(value)
        .expect_err("runtimeContract is mandatory");
    assert!(error.to_string().contains("runtimeContract"));
}

#[test]
fn provider_runtime_contract_rejects_duplicate_operations() {
    let mut value = canonical_manifest_value();
    let duplicate = value["runtimeContract"]["operations"][0].clone();
    value["runtimeContract"]["operations"]
        .as_array_mut()
        .expect("runtime operations")
        .push(duplicate);
    let manifest = serde_json::from_value(value).expect("deserialize duplicate operation fixture");
    assert!(
        validate_provider_manifest_contract(&manifest)
            .iter()
            .any(|error| error.contains("duplicate operation"))
    );
}

#[test]
fn provider_runtime_contract_requires_all_resident_operations() {
    let mut value = canonical_manifest_value();
    value["runtimeContract"]["operations"]
        .as_array_mut()
        .expect("runtime operations")
        .retain(|operation| operation["operation"] != "projection-batch-stdin");
    let manifest = serde_json::from_value(value).expect("deserialize incomplete operation fixture");
    assert!(
        validate_provider_manifest_contract(&manifest)
            .iter()
            .any(|error| error.contains("projection-batch-stdin"))
    );
}

#[test]
fn julia_is_server_only_and_owns_one_http_runtime_contract() {
    let value = builtin_provider_manifests()
        .into_iter()
        .map(|manifest| serde_json::to_value(manifest).expect("serialize provider manifest"))
        .find(|manifest| manifest["languageId"] == "julia")
        .expect("Julia provider manifest");
    assert_eq!(value["runtimeContract"]["transport"], "http-json");
    assert_eq!(
        value["runtimeContract"]["aspClientServer"]["command"],
        serde_json::json!(["serve"])
    );
    assert_eq!(
        value["runtimeContract"]["aspClientServer"]["warmupPolicy"],
        "before-ready"
    );
    assert!(value.get("languageProjection").is_none());
    assert_eq!(
        value["runtimeContract"]["operations"],
        serde_json::json!([
            {
                "operation": "projection-batch-stdin",
                "requestSchemaId": "https://schemas.agent-semantic-protocols.dev/provider-language-projection-batch-request.v1.schema.json",
                "responseSchemaId": "https://schemas.agent-semantic-protocols.dev/provider-language-projection-batch-response.v1.schema.json"
            },
            {
                "operation": "project-resolution-stdin",
                "requestSchemaId": "https://schemas.agent-semantic-protocols.dev/provider-project-resolution-request.v1.schema.json",
                "responseSchemaId": "https://schemas.agent-semantic-protocols.dev/provider-project-resolution-response.v1.schema.json"
            }
        ])
    );
}
