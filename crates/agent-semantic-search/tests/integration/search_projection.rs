use agent_semantic_search::{
    GraphSearchProjectionRenderer, SearchProjectionDensityV1, SearchProjectionRenderer,
    SearchProjectionRequestV1, SemanticSearchPacketV1,
};
use serde_json::json;

fn packet() -> SemanticSearchPacketV1 {
    SemanticSearchPacketV1::from_value(json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "view": "owner",
        "query": "src/lib.rs",
        "items": [],
        "owners": [],
        "nextActions": []
    }))
    .expect("valid packet")
}

#[test]
fn density_does_not_change_semantic_digest() {
    let packet = packet();
    let renderer = GraphSearchProjectionRenderer;
    let terse = renderer
        .render(
            &packet,
            &SearchProjectionRequestV1::new("owner", SearchProjectionDensityV1::Terse),
        )
        .expect("terse projection");
    let expanded = renderer
        .render(
            &packet,
            &SearchProjectionRequestV1::new("owner", SearchProjectionDensityV1::Expanded),
        )
        .expect("expanded projection");

    assert_eq!(terse.semantic_digest, expanded.semantic_digest);
}

#[test]
fn request_rejects_unknown_fields() {
    let error = serde_json::from_value::<SearchProjectionRequestV1>(json!({
        "schemaId": "asp.search-projection-request.v1",
        "schemaVersion": "v1",
        "projectionId": "owner",
        "density": "standard",
        "compact": true
    }))
    .expect_err("legacy compact field must be rejected");

    assert!(error.to_string().contains("unknown field"));
}
