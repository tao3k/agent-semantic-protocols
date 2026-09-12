// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_context_product::ContextProductEvent;
use agent_semantic_context_product::ExecutionConsumed;
use agent_semantic_context_product::ExecutionRevoked;
use agent_semantic_context_product::ExecutionStarted;
use agent_semantic_context_product::StateAuthorityReceipt;
use serde_json::Value;
use serde_json::json;

fn digest(byte: char) -> String {
    format!(
        "blake3:{}",
        std::iter::repeat_n(byte, 64).collect::<String>()
    )
}

fn common_event(event_type: &str) -> Value {
    json!({
        "eventType": event_type,
        "eventId": "event-1",
        "runId": "run-1",
        "sequence": 1,
        "stateRevision": 0,
        "preStateDigest": digest('a'),
        "eventDigest": digest('f')
    })
}

#[test]
fn v1_execution_events_deserialize_from_the_shared_wire_shape() {
    let mut started = common_event("ExecutionStarted");
    let started = started.as_object_mut().expect("object");
    started.extend(
        json!({
            "grantId": "grant-1",
            "grantDigest": digest('b'),
            "attemptId": "attempt-1",
            "attemptDigest": digest('c'),
            "executionGroupId": "execution-group-1",
            "stageIds": ["stage-1"],
            "providerId": "provider-1",
            "operation": "search",
            "leaseFence": 1,
            "expiresAtMs": 1000,
            "providerIdempotencyKey": "provider-key-1"
        })
        .as_object()
        .expect("object")
        .clone(),
    );

    let mut consumed = common_event("ExecutionConsumed");
    let consumed = consumed.as_object_mut().expect("object");
    consumed.extend(
        json!({
            "grantId": "grant-1",
            "grantDigest": digest('b'),
            "attemptId": "attempt-1",
            "executionGroupId": "execution-group-1",
            "stageIds": ["stage-1"],
            "providerId": "provider-1",
            "operation": "search",
            "resultReceiptRef": "result-1",
            "resultDigest": digest('d'),
            "leaseFence": 1
        })
        .as_object()
        .expect("object")
        .clone(),
    );

    let mut revoked = common_event("ExecutionRevoked");
    let revoked = revoked.as_object_mut().expect("object");
    revoked.extend(
        json!({
            "reasonCode": "lease-expired",
            "executionGroupId": "execution-group-1",
            "stageIds": ["stage-1"],
            "providerId": "provider-1",
            "operation": "search",
            "admissionId": "admission-1",
            "grantId": "grant-1",
            "actionKey": digest('e')
        })
        .as_object()
        .expect("object")
        .clone(),
    );

    let started = Value::Object(started.clone());
    let consumed = Value::Object(consumed.clone());
    let revoked = Value::Object(revoked.clone());

    serde_json::from_value::<ExecutionStarted>(started.clone())
        .expect("ExecutionStarted must match the v1 schema");
    serde_json::from_value::<ExecutionConsumed>(consumed.clone())
        .expect("ExecutionConsumed must match the v1 schema");
    serde_json::from_value::<ExecutionRevoked>(revoked.clone())
        .expect("ExecutionRevoked must match the v1 schema");

    for event in [started, consumed, revoked] {
        serde_json::from_value::<ContextProductEvent>(event)
            .expect("the v1 event union and Rust wire type must agree");
    }
}

#[test]
fn v1_state_authority_receipt_deserializes_from_the_shared_wire_shape() {
    let receipt = json!({
        "receiptId": "authority-receipt-1",
        "authorityId": "client-db-1",
        "runId": "run-1",
        "revision": 1,
        "stateDigest": digest('a'),
        "eventLogDigest": digest('b'),
        "issuedAtMs": 1000,
        "receiptDigest": digest('c')
    });

    serde_json::from_value::<StateAuthorityReceipt>(receipt)
        .expect("the v1 authority receipt schema and Rust wire type must agree");
}
