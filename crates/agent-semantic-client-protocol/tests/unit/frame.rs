// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{ClientResponsePayload, ClientResponsePayloadInner};

#[test]
fn response_payload_clone_shares_storage_and_keeps_wire_json_transparent() {
    let value = serde_json::json!({"owners": ["src/lib.rs"], "state": "ready"});
    let payload = ClientResponsePayload::from(value.clone());
    let cloned = payload.clone();

    let (
        ClientResponsePayloadInner::Shared(payload_value),
        ClientResponsePayloadInner::Shared(cloned_value),
    ) = (&payload.0, &cloned.0)
    else {
        panic!("plain response payload must retain shared storage");
    };
    assert!(std::sync::Arc::ptr_eq(payload_value, cloned_value));
    assert_eq!(
        serde_json::to_value(&payload).expect("serialize shared response payload"),
        value
    );
    let restored: ClientResponsePayload =
        serde_json::from_value(value.clone()).expect("deserialize shared response payload");
    assert_eq!(restored.as_value(), &value);
}

#[test]
fn response_object_overlay_keeps_template_shared_and_is_wire_transparent() {
    let template = std::sync::Arc::new(serde_json::json!({
        "requestId": "template",
        "requestProfile": "materialized",
        "nested": {"owners": ["src/lib.rs"]}
    }));
    let original = template.clone();
    let fields = std::collections::BTreeMap::from([
        ("requestId".to_owned(), serde_json::json!("current")),
        (
            "requestProfile".to_owned(),
            serde_json::json!("resident-hit"),
        ),
        (
            "requestPlaneElapsedMicros".to_owned(),
            serde_json::json!(42),
        ),
    ]);
    let payload = ClientResponsePayload::from_shared_object_overlay(template, fields)
        .expect("construct object overlay");
    let ClientResponsePayloadInner::ObjectOverlay {
        template: retained, ..
    } = &payload.0
    else {
        panic!("response payload must retain an overlay");
    };
    assert!(std::sync::Arc::ptr_eq(&original, retained));

    let wire = serde_json::to_value(&payload).expect("serialize response overlay");
    assert_eq!(wire["requestId"], "current");
    assert_eq!(wire["requestProfile"], "resident-hit");
    assert_eq!(wire["requestPlaneElapsedMicros"], 42);
    assert_eq!(wire["nested"], original["nested"]);
    assert_eq!(original["requestId"], "template");

    let restored: ClientResponsePayload =
        serde_json::from_value(wire.clone()).expect("deserialize response overlay");
    assert_eq!(restored.into_value(), wire);
}
