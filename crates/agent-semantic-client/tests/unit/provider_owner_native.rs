use super::{
    ExpectedOwnerResponse, ProviderNativeOwnerSearchResponse, validate_provider_owner_response,
};

fn response(projection: serde_json::Value) -> ProviderNativeOwnerSearchResponse {
    serde_json::from_value(serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-native-owner-search-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "requestedOwnerPath": "src/lib.rs",
        "requestedProjectionMode": "complete-owner",
        "sourceContentDigest": "a".repeat(64),
        "parsedOwnerCount": 1,
        "projectionCompleteness": "complete-owner",
        "projections": [projection]
    }))
    .expect("provider owner response")
}

fn method_projection() -> serde_json::Value {
    serde_json::json!({
        "canonicalItemSelector": {
            "schemaId": "asp.canonical-item-selector.v1",
            "schemaVersion": "1",
            "languageId": "rust",
            "kind": "method",
            "symbol": "render",
            "scopes": [
                {"relation": "implementation-owner", "kind": "type", "symbol": "Widget"},
                {"relation": "trait-owner", "kind": "trait", "symbol": "Render"}
            ],
            "structuralSelector": "rust://src/lib.rs#item/method/render/scope/implementation-owner/type/Widget/scope/trait-owner/trait/Render"
        },
        "signature": "fn render(&self)",
        "captureName": "declaration.name",
        "sourceByteStart": 0,
        "sourceByteEnd": 16
    })
}

#[test]
fn canonical_selector_packet_is_the_only_provider_projection_identity() {
    let projections = validate_provider_owner_response(
        response(method_projection()),
        ExpectedOwnerResponse {
            language_id: "rust",
            provider_id: "rs-harness",
            owner_path: "src/lib.rs",
            projection_mode: "complete-owner",
            content_digest: &"a".repeat(64),
            source_size: 16,
        },
    )
    .expect("canonical provider projection");
    assert_eq!(projections.len(), 1);
    assert_eq!(projections[0].item_kind, "method");
    assert_eq!(projections[0].item_name, "render");
    assert!(
        projections[0]
            .structural_selector
            .contains("implementation-owner/type/Widget")
    );
    assert!(
        projections[0]
            .structural_selector
            .contains("trait-owner/trait/Render")
    );
}

#[test]
fn duplicated_legacy_projection_identity_is_rejected() {
    let mut projection = method_projection();
    projection["structuralSelector"] = serde_json::json!("rust://src/lib.rs#item/method/render");
    projection["itemKind"] = serde_json::json!("function");
    projection["itemName"] = serde_json::json!("render");
    let encoded = serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-native-owner-search-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "requestedOwnerPath": "src/lib.rs",
        "requestedProjectionMode": "complete-owner",
        "sourceContentDigest": "a".repeat(64),
        "parsedOwnerCount": 1,
        "projectionCompleteness": "complete-owner",
        "projections": [projection]
    });
    let error = serde_json::from_value::<ProviderNativeOwnerSearchResponse>(encoded)
        .expect_err("legacy identity fields must not deserialize");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn typed_identity_and_structural_selector_drift_is_rejected() {
    let mut projection = method_projection();
    projection["canonicalItemSelector"]["kind"] = serde_json::json!("function");
    let error = validate_provider_owner_response(
        response(projection),
        ExpectedOwnerResponse {
            language_id: "rust",
            provider_id: "rs-harness",
            owner_path: "src/lib.rs",
            projection_mode: "complete-owner",
            content_digest: &"a".repeat(64),
            source_size: 16,
        },
    )
    .expect_err("typed selector drift must fail closed");
    assert!(
        error.contains("identity does not match typed identity"),
        "{error}"
    );
}
