// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::CALLABLE_SKELETON_PAYLOAD_SCHEMA_ID;
use super::CallableSkeletonPayload;
use crate::semantic_projection::SemanticProjection;

#[test]
fn python_provider_projection_deserializes_through_shared_contract() {
    let payload = serde_json::json!({
        "rootSelector": "python://fixture.py#item/function/f",
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
        "payloadDigest": "blake3-256:50510f37ae65932eb4efbf0cddd2a9047dda18cfead846d4afbeabc68bf03bb6",
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

#[test]
fn shared_callable_fixture_has_valid_digest_nodes_and_root_scope() {
    let decoded: SemanticProjection<CallableSkeletonPayload> = serde_json::from_str(include_str!(
        "../../../../../schemas/fixtures/semantic-projection.callable-skeleton.v1.json"
    ))
    .unwrap();
    decoded.validate().unwrap();
    decoded.payload.validate().unwrap();
    decoded
        .payload
        .validate_scope(decoded.root_selector.as_str())
        .unwrap();
}

#[test]
fn callable_payload_cannot_omit_its_v1_root() {
    let mut fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/fixtures/semantic-projection.callable-skeleton.v1.json"
    ))
    .unwrap();
    fixture["payload"]
        .as_object_mut()
        .unwrap()
        .remove("rootSelector");
    assert!(serde_json::from_value::<CallableSkeletonPayload>(fixture["payload"].clone()).is_err());
}
