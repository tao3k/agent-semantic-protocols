use super::{CALLABLE_SKELETON_PAYLOAD_SCHEMA_ID, CallableSkeletonPayload};
use crate::semantic_projection::SemanticProjection;

#[test]
fn python_provider_projection_deserializes_through_shared_contract() {
    let payload = serde_json::json!({
        "rootNodeId": "callable:root",
        "callable": {"kind": "function", "displayName": "f", "signature": "f"},
        "nodes": [{
            "nodeId": "callable:root",
            "kind": "callable",
            "label": "f",
            "order": 0,
            "queryable": true,
            "selector": "python://fixture.py#item/function/f",
            "languageFacts": {"async": false, "decoratorCount": 0, "inputCount": 0}
        }],
        "relations": [],
        "cost": {"sourceBytes": 21, "projectedBytes": 21, "omittedBytes": 0},
        "languageFacts": {"parser": "ast", "syntax": "python"}
    });
    let projection = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-projection",
        "schemaVersion": "1",
        "projectionKind": "callable-skeleton",
        "languageId": "python",
        "providerId": "asp-python",
        "rootSelector": "python://fixture.py#item/function/f",
        "evidenceContextRef": "blake3-256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "payloadSchemaId": CALLABLE_SKELETON_PAYLOAD_SCHEMA_ID,
        "payloadDigest": "blake3-256:bf6d7b55bfebaeb49abb52358b2feca1b2f41babc44e13231f52ca1c724a5983",
        "payload": payload
    });
    let decoded: SemanticProjection<CallableSkeletonPayload> =
        serde_json::from_value(projection).expect("Python projection must match shared contract");
    decoded
        .validate()
        .expect("Python semantic projection must validate");
    assert_eq!(decoded.language_id, "python");
    assert_eq!(decoded.provider_id, "asp-python");
}
