use agent_semantic_client::projection_presentation::ProjectionPresentation;
use agent_semantic_client::projection_presentation::render_exact_projection_response;
use agent_semantic_client_protocol::ClientFrame;
use serde_json::Value;
use serde_json::json;

fn response(result: Value) -> ClientFrame {
    serde_json::from_value(json!({
        "kind": "response",
        "schemaId": "agent.semantic-protocols.client.frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sessionId": "session-projection",
        "projectId": "repo-projection",
        "workspaceId": "workspace-projection",
        "requestId": "request-projection",
        "outcome": "ready",
        "result": result,
        "error": null,
        "catalog": null
    }))
    .expect("typed response fixture")
}

#[test]
fn renders_provider_projection_bytes_as_utf8_text() {
    let frame = response(json!({
        "result": {"bytes": [112, 117, 98, 32, 102, 110, 32, 114, 101, 97, 100, 121, 40, 41, 32, 123, 125]}
    }));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap(),
        "pub fn ready() {}"
    );
}

#[test]
fn rejects_non_byte_projection_values() {
    let frame = response(json!({"result": {"bytes": [256]}}));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err(),
        "exact projection response contains a non-byte value"
    );
}

#[test]
fn does_not_expand_an_owner_repair_packet_as_an_exact_projection() {
    let frame = response(json!({
        "result": {
            "state": "owner-for-repair",
            "owner": {"bytes": [101, 110, 116, 105, 114, 101, 32, 111, 119, 110, 101, 114]}
        }
    }));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err(),
        "exact projection response has no text or byte payload"
    );
}

#[test]
fn preserves_runtime_exact_query_failure_terminal() {
    let frame: ClientFrame = serde_json::from_value(json!({
        "kind": "response",
        "schemaId": "agent.semantic-protocols.client.frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sessionId": "session-projection",
        "projectId": "repo-projection",
        "workspaceId": "workspace-projection",
        "requestId": "request-projection",
        "outcome": "error",
        "result": null,
        "error": {
            "reasonKind": "projection-missing",
            "message": "exact projection terminal: projection-missing",
            "terminal": {
                "schemaId": "agent.semantic-protocols.asp-client-exact-query-failure",
                "schemaVersion": "1",
                "state": "failed",
                "operationId": "request-projection",
                "projectId": "repo-projection",
                "workspaceId": "workspace-projection",
                "languageId": "rust",
                "providerId": "asp-rust",
                "requestedSelector": "rust://src/lib.rs#item/function/missing",
                "resolvedSelector": "rust://src/lib.rs#item/function/missing",
                "projectionKind": "source",
                "phase": "resident-selector-read",
                "reasonKind": "projection-missing",
                "generationDigest": format!("blake3-256:{}", "a".repeat(64)),
                "rootDigest": "b".repeat(64),
                "recommendedNext": {"action": "query-owner-or-admitted-scope"},
                "residentReadElapsedMicros": 7,
                "serviceElapsedMicros": 3,
                "elapsedMicros": 10,
                "workCounters": {
                    "databaseReadCount": 0,
                    "filesystemReadCount": 0,
                    "providerProcessCount": 0,
                    "schedulerTaskCount": 0,
                    "socketOperationCount": 0
                },
                "details": {"selectorState": "projection-missing"}
            }
        },
        "catalog": null
    }))
    .expect("typed failure frame");

    let rendered =
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err();
    assert!(rendered.contains("\"reasonKind\":\"projection-missing\""));
    assert!(rendered.contains("\"phase\":\"resident-selector-read\""));
    assert!(rendered.contains("\"serviceElapsedMicros\":3"));
    assert!(rendered.contains("\"recommendedNext\""));
}

#[test]
fn text_and_machine_presentations_share_one_typed_response() {
    let frame = response(json!({
        "result": {"bytes": [102, 110, 32, 109, 97, 105, 110, 40, 41, 32, 123, 125]}
    }));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap(),
        "fn main() {}"
    );
    let machine =
        render_exact_projection_response(&frame, ProjectionPresentation::MachineJson).unwrap();
    assert!(machine.contains("\"bytes\":[102,110,32,109,97,105,110"));
}
