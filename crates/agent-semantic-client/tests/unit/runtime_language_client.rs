use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientOutcome, ClientRequestId, ClientSessionId, ClientWorkspaceIdentity,
    SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID, SCHEMA_VERSION, SchemaBundleEntry, SchemaBundleReceipt,
    SchemaBundleResponse,
};

use crate::runtime_language_client::decode_schema_bundle_response;

#[test]
fn typed_schema_bundle_decoder_preserves_failed_terminal() {
    let response = SchemaBundleResponse::Failed {
        schema_id: SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        language_id: "unknown".to_owned(),
        reason_kind: "schema-bundle-language-unregistered".to_owned(),
        recommended_next: serde_json::json!({"action": "select-registered-language-profile"}),
        details: serde_json::json!({}),
    };
    let frame = ClientFrame::Response {
        base: ClientFrameBase {
            schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
            protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
            session_id: ClientSessionId::new("test-session").expect("session id"),
            workspace_identity: ClientWorkspaceIdentity::new("workspace")
                .expect("workspace identity"),
            trace_context: None,
        },
        request_id: ClientRequestId::new("schema-request").expect("request id"),
        outcome: ClientOutcome::Ready,
        result: Some(serde_json::to_value(&response).expect("response JSON")),
        error: None,
        catalog: None,
    };
    let decoded = decode_schema_bundle_response(frame).expect("typed response");
    assert_eq!(decoded, response);
}

#[test]
fn typed_schema_bundle_unchanged_validates_entry_identity() {
    let entry = SchemaBundleEntry {
        family_id: "client-protocol".to_owned(),
        schema_id: "https://schemas.example/schema.json".to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        name: "schema.schema.json".to_owned(),
        digest: format!("blake3-256:{}", "a".repeat(64)),
    };
    let response = SchemaBundleResponse::Unchanged {
        schema_id: SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        receipt: SchemaBundleReceipt {
            language_id: "rust".to_owned(),
            root_set_ids: vec!["client-protocol".to_owned()],
            bundle_digest: format!("blake3-256:{}", "b".repeat(64)),
        },
        entries: vec![entry],
    };
    response.validate().expect("valid typed response");
}
