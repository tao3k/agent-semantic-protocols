use agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload;
use agent_semantic_content_identity::semantic_projection::SemanticProjection;
use agent_semantic_content_identity::semantic_projection::SemanticProjectionInput;

fn response() -> SemanticProjection<CallableSkeletonPayload> {
    let mut fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/semantic-projection.callable-skeleton.v1.json"
    ))
    .expect("decode callable projection fixture");
    let root_selector = fixture["rootSelector"]
        .as_str()
        .expect("fixture root selector")
        .to_owned();
    fixture = fixture["payload"].clone();
    let payload: CallableSkeletonPayload =
        serde_json::from_value(fixture).expect("decode callable skeleton payload");
    SemanticProjection::new(SemanticProjectionInput {
        projection_kind: CallableSkeletonPayload::projection_kind().into(),
        language_id: "rust".into(),
        provider_id: "asp-rust".into(),
        root_selector: root_selector.into(),
        evidence_context_ref:
            "blake3-256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into(),
        payload_schema_id: "agent.semantic-protocols.callable-skeleton".into(),
        payload,
    })
    .expect("decode typed semantic projection")
}

#[test]
fn resolved_response_accepts_matching_authority() {
    let projection = response();
    projection
        .validate()
        .expect("matching semantic projection authority must validate");
    assert_eq!(projection.language_id, "rust");
    assert_eq!(projection.provider_id, "asp-rust");
}

#[test]
fn resolved_response_names_the_exact_drifting_authority_field() {
    let projection = response();
    let expected_root = "rust://crates/example/src/other.rs#node/function/run/identity=root";
    assert_ne!(projection.root_selector, expected_root);
    assert_eq!(
        projection.root_selector,
        "rust://crates/example/src/lib.rs#item/function/run"
    );
}
