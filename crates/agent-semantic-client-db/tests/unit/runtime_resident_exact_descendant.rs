// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{CallableSkeletonPayload, SemanticProjection, descendant_range};
use agent_semantic_content_identity::semantic_projection::SemanticProjectionInput;

const ROOT: &str = "rust://src/lib.rs#item/function/run";
const CHILD: &str = "rust://src/lib.rs#item/function/run/segment/branch/ordinal-1";

fn envelope(mut update: impl FnMut(&mut serde_json::Value)) -> Vec<u8> {
    let mut payload = serde_json::json!({
        "rootSelector": ROOT, "rootNodeId": "root",
        "callable": {"kind": "function", "displayName": "run", "signature": "run"},
        "nodes": [
            {"nodeId": "root", "kind": "callable", "label": "run", "order": 0, "queryable": false},
            {"nodeId": "child", "kind": "branch", "label": "if", "order": 1,
             "queryable": true, "selector": CHILD,
             "sourceLocatorHint": {"sourceByteStart": 15, "sourceByteEnd": 30}}
        ],
        "relations": [], "cost": {"sourceBytes": 50, "projectedBytes": 50, "omittedBytes": 0}
    });
    update(&mut payload);
    let payload: CallableSkeletonPayload = serde_json::from_value(payload).unwrap();
    let envelope = SemanticProjection::new(SemanticProjectionInput {
        projection_kind: "callable-skeleton".into(),
        language_id: "rust".into(),
        provider_id: "asp-rust".into(),
        root_selector: ROOT.into(),
        evidence_context_ref: format!("blake3-256:{}", "a".repeat(64)).into(),
        payload_schema_id: "agent.semantic-protocols.callable-skeleton".into(),
        payload,
    })
    .unwrap();
    serde_json::to_vec(&envelope).unwrap()
}

#[test]
fn exact_descendant_requires_materialized_identity_and_range() {
    assert_eq!(
        descendant_range(&envelope(|_| {}), ROOT, CHILD).unwrap(),
        15..30
    );
    assert!(
        descendant_range(
            &envelope(|_| {}),
            ROOT,
            &format!("{ROOT}/segment/branch/ordinal-99")
        )
        .is_err()
    );
    assert!(
        descendant_range(
            &envelope(|payload| {
                payload["nodes"][1]["queryable"] = false.into();
            }),
            ROOT,
            CHILD
        )
        .is_err()
    );
    assert!(
        descendant_range(
            &envelope(|payload| {
                payload["nodes"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("sourceLocatorHint");
            }),
            ROOT,
            CHILD
        )
        .is_err()
    );
}

#[test]
fn exact_descendant_rejects_digest_and_root_substitution() {
    let bytes = envelope(|_| {});
    assert!(descendant_range(&bytes, "rust://src/lib.rs#item/function/other", CHILD).is_err());
    let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    forged["payload"]["nodes"][1]["sourceLocatorHint"]["sourceByteStart"] = 1.into();
    assert!(descendant_range(&serde_json::to_vec(&forged).unwrap(), ROOT, CHILD).is_err());
}
