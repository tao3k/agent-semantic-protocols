// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_protocol::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID;
use agent_semantic_client_protocol::SchemaBundleEntry;
use agent_semantic_client_protocol::SchemaBundleReceipt;
use agent_semantic_client_protocol::SchemaBundleResponse;
use agent_semantic_client_protocol::protocol_identity::CLIENT_FRAME_SCHEMA_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_VERSION;
use agent_semantic_client_protocol::protocol_identity::SCHEMA_VERSION;

use crate::runtime_language_client::AspClient;
use crate::runtime_language_client::SessionKey;
use crate::runtime_language_client::SessionRegistry;
use crate::runtime_language_client::decode_schema_bundle_response;
use crate::runtime_language_client::session_for_key;
use crate::runtime_language_client::validate_cancelled_terminal;

#[cfg(unix)]
#[test]
fn absent_host_descriptor_uses_the_verified_published_endpoint_path() {
    let client = AspClient::new_from_host_descriptor_value("/state", "/workspace", None)
        .expect("an absent optional descriptor must not block normal CLI transport");
    assert!(client.uses_published_loopback_transport());
}

fn client_frame_base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("test-session").expect("session id"),
        project_id: agent_semantic_client_protocol::ClientProjectId::new("repo-project")
            .expect("project id"),
        workspace_id: ClientWorkspaceIdentity::new("workspace").expect("workspace identity"),
        trace_context: None,
    }
}

fn cancellation_response(
    request_id: ClientRequestId,
    error: Option<serde_json::Value>,
) -> ClientFrame {
    ClientFrame::Response {
        base: client_frame_base(),
        request_id,
        outcome: ClientOutcome::Cancelled,
        result: None,
        error,
        catalog: None,
    }
}

#[test]
fn cancelled_outcome_with_typed_diagnostic_is_one_terminal() {
    let request_id = ClientRequestId::new("cancel-request").expect("request id");
    validate_cancelled_terminal(
        cancellation_response(
            request_id.clone(),
            Some(serde_json::json!({
                "reasonKind": "client-request-cancelled",
                "message": "client request was cancelled",
            })),
        ),
        &request_id,
    )
    .expect("typed cancellation terminal");
}

#[test]
fn bare_cancelled_outcome_is_not_a_typed_terminal() {
    let request_id = ClientRequestId::new("cancel-request").expect("request id");
    let error =
        validate_cancelled_terminal(cancellation_response(request_id.clone(), None), &request_id)
            .expect_err("bare Cancelled must fail closed");
    assert!(error.contains("typed diagnostic"));
}

#[test]
fn cancellation_diagnostic_cannot_be_attached_to_another_request() {
    let request_id = ClientRequestId::new("cancel-request").expect("request id");
    let other_request_id = ClientRequestId::new("other-request").expect("request id");
    let error = validate_cancelled_terminal(
        cancellation_response(
            other_request_id,
            Some(serde_json::json!({
                "reasonKind": "client-request-cancelled",
                "message": "client request was cancelled",
            })),
        ),
        &request_id,
    )
    .expect_err("cross-request cancellation must fail closed");
    assert!(error.contains("request identity mismatch"));
}

#[tokio::test]
async fn thirty_two_concurrent_misses_share_one_single_flight_connection() {
    struct TestSession {
        initialized: tokio::sync::OnceCell<()>,
    }

    let registry = std::sync::Arc::new(tokio::sync::Mutex::new(
        SessionRegistry::<TestSession>::new(32),
    ));
    let key = SessionKey::fixture(1);
    let connects = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let initializes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(32));
    let mut tasks = tokio::task::JoinSet::new();

    for _ in 0..32 {
        let registry = std::sync::Arc::clone(&registry);
        let key = key.clone();
        let connects = std::sync::Arc::clone(&connects);
        let initializes = std::sync::Arc::clone(&initializes);
        let barrier = std::sync::Arc::clone(&barrier);
        tasks.spawn(async move {
            barrier.wait().await;
            let (session, _) = session_for_key(registry.as_ref(), &key, || async {
                connects.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tokio::task::yield_now().await;
                Ok(std::sync::Arc::new(TestSession {
                    initialized: tokio::sync::OnceCell::new(),
                }))
            })
            .await
            .expect("one shared connection");
            session
                .initialized
                .get_or_init(|| async {
                    initializes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tokio::task::yield_now().await;
                })
                .await;
        });
    }

    while let Some(result) = tasks.join_next().await {
        result.expect("single-flight task");
    }
    assert_eq!(connects.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(initializes.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(registry.lock().await.len(), 1);
}

#[tokio::test]
async fn registry_is_bounded_and_never_evicts_an_active_or_connecting_lease() {
    let mut registry = SessionRegistry::<usize>::new(1);
    let connecting = registry
        .reserve(SessionKey::fixture(10))
        .expect("first connecting entry");
    let error = registry
        .reserve(SessionKey::fixture(11))
        .expect_err("connecting entry consumes bounded capacity");
    assert!(error.contains("runtime-client-session-capacity-exhausted"));

    let active = connecting
        .get_or_init(|| async { std::sync::Arc::new(10) })
        .await
        .clone();
    let error = registry
        .reserve(SessionKey::fixture(12))
        .expect_err("active lease cannot be evicted");
    assert!(error.contains("activeOrConnecting=1"));

    drop(active);
    assert_eq!(registry.drain_idle(), 1);
    assert!(registry.is_empty());
}

#[tokio::test]
async fn publication_nonce_change_cannot_reuse_the_previous_session() {
    let mut registry = SessionRegistry::<usize>::new(2);
    let first_key = SessionKey::fixture(20);
    let next_key = SessionKey::fixture_successor(20);

    let first = registry.reserve(first_key).expect("first generation");
    first
        .get_or_init(|| async { std::sync::Arc::new(20) })
        .await;
    let successor = registry
        .reserve(next_key)
        .expect("successor generation has a distinct entry");

    assert!(!std::sync::Arc::ptr_eq(&first, &successor));
    assert_eq!(registry.len(), 2);
}

#[tokio::test]
async fn exact_workspace_session_eviction_does_not_drain_another_workspace() {
    let mut registry = SessionRegistry::<usize>::new(2);
    let rust = SessionKey::fixture(30);
    let python = SessionKey::fixture(31);
    registry.reserve(rust.clone()).expect("Rust session");
    registry.reserve(python).expect("Python session");
    assert!(registry.remove_key(&rust));
    assert_eq!(registry.len(), 1);
    assert!(!registry.remove_key(&rust));
}

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
            project_id: agent_semantic_client_protocol::ClientProjectId::new("repo-project")
                .expect("project id"),
            workspace_id: ClientWorkspaceIdentity::new("workspace").expect("workspace identity"),
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
