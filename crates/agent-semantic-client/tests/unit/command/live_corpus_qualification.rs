use super::parse_args;
use crate::command::live_corpus::qualification::client_protocol::{
    PublicRouteTerminal, typed_terminal,
};

fn error_frame(
    reason_kind: &str,
    admission_state: &str,
) -> agent_semantic_client_protocol::ClientFrame {
    serde_json::from_value(serde_json::json!({
        "kind": "response",
        "schemaId": "agent.semantic-protocols.client-frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sessionId": "live-corpus-session",
        "workspaceIdentity": "live-corpus-workspace",
        "requestId": "live-corpus-request",
        "outcome": "error",
        "error": {
            "reasonKind": reason_kind,
            "message": "generation is not ready",
            "details": {
                "schemaId": "agent.semantic-protocols.asp-client-exact-query-failure",
                "schemaVersion": "1",
                "state": "failed",
                "operationId": "live-corpus-request",
                "languageId": "rust",
                "providerId": "asp-rust",
                "requestedSelector": "rust:item:test",
                "resolvedSelector": null,
                "projectionKind": "source",
                "phase": "workspace-generation-admission",
                "reasonKind": reason_kind,
                "generationDigest": null,
                "rootDigest": null,
                "recommendedNext": {"action": "observe-runtime-generation"},
                "residentReadElapsedMicros": 0,
                "serviceElapsedMicros": 1,
                "elapsedMicros": 1,
                "workCounters": {
                    "databaseReadCount": 0,
                    "filesystemReadCount": 0,
                    "providerProcessCount": 0,
                    "schedulerTaskCount": 0,
                    "socketOperationCount": 0
                },
                "details": {"admissionState": admission_state}
            }
        }
    }))
    .expect("typed ASP Client error frame fixture")
}

#[test]
fn persistent_queued_is_a_typed_rejecting_terminal() {
    let terminal = typed_terminal(error_frame("runtime-generation-queued", "Queued"))
        .expect("typed Queued terminal");
    assert!(matches!(terminal, PublicRouteTerminal::Queued(_)));
}

#[test]
fn persistent_building_is_a_typed_rejecting_terminal() {
    let terminal = typed_terminal(error_frame("runtime-generation-building", "Building"))
        .expect("typed Building terminal");
    assert!(matches!(terminal, PublicRouteTerminal::Building(_)));
}

#[test]
fn qualification_accepts_one_explicit_resource_selector() {
    let args = parse_args(&["--resource".to_owned(), "rust.bytes".to_owned()])
        .expect("parse one resource selector");
    assert_eq!(args.resource_id.as_deref(), Some("rust.bytes"));
}

#[test]
fn qualification_rejects_a_missing_resource_selector_value() {
    let error =
        parse_args(&["--resource".to_owned()]).expect_err("resource selector value is required");
    assert!(error.contains("requires a resource id after --resource"));
}

#[test]
fn qualification_rejects_duplicate_resource_selectors() {
    let error = parse_args(&[
        "--resource".to_owned(),
        "rust.bytes".to_owned(),
        "--resource".to_owned(),
        "rust.tokio".to_owned(),
    ])
    .expect_err("one atomic resource selector is allowed");
    assert!(error.contains("accepts exactly one --resource option"));
}
