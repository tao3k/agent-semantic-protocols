use agent_semantic_content_identity::callable_skeleton_projection::{
    ProviderNativeExactAuthority, ProviderNativeExactProjection,
};

fn response() -> ProviderNativeExactProjection {
    let projection: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/callable-skeleton-projection/valid-rust-dispatch.v1.json"
    ))
    .expect("decode callable projection fixture");
    serde_json::from_value(serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-native-exact-projection",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "ownerPath": "crates/example/src/dispatch.rs",
        "requestedStructuralSelector": "rust://crates/example/src/dispatch.rs#item/function/run",
        "structuralSelector": "rust://crates/example/src/dispatch.rs#node/function/run/identity=root",
        "projectionMode": "callable-skeleton",
        "normalizedParserFacts": {},
        "projectionPayload": projection,
        "sourceContentDigest": "d".repeat(64),
        "sourceByteStart": 0,
        "sourceByteEnd": 100
    }))
    .expect("decode typed provider exact response")
}

#[test]
fn resolved_response_accepts_matching_authority() {
    response()
        .validate_authority(ProviderNativeExactAuthority {
            language_id: "rust",
            provider_id: "rs-harness",
            owner_path: "crates/example/src/dispatch.rs",
            requested_structural_selector:
                "rust://crates/example/src/dispatch.rs#item/function/run",
        })
        .expect("matching authority must validate");
}

#[test]
fn resolved_response_names_the_exact_drifting_authority_field() {
    let error = response()
        .validate_authority(ProviderNativeExactAuthority {
            language_id: "rust",
            provider_id: "rs-harness",
            owner_path: "crates/example/src/other.rs",
            requested_structural_selector:
                "rust://crates/example/src/dispatch.rs#item/function/run",
        })
        .expect_err("owner drift must fail closed");

    assert_eq!(
        error,
        "provider-native exact response ownerPath mismatch: expected=crates/example/src/other.rs actual=crates/example/src/dispatch.rs"
    );
}
