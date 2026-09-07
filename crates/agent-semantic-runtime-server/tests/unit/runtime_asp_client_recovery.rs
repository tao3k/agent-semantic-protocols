// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_server::AspClientResponseTelemetry;

use super::RuntimeProjectWorkspaceKey;
use super::RuntimeQueryGenerationState;
use super::record_runtime_response_serialized;
use super::record_runtime_terminal_egressed;
use super::service::ClientRequestKey;
use super::wait_for_runtime_query_generation;
use super::wait_for_runtime_query_generation_change;

fn key() -> RuntimeProjectWorkspaceKey {
    RuntimeProjectWorkspaceKey::new(
        ClientProjectId::new("project-first-search").expect("project id"),
        ClientWorkspaceIdentity::new("workspace-first-search").expect("workspace id"),
    )
}

#[tokio::test]
async fn absent_first_search_waits_only_for_the_new_byte_generation_terminal() {
    let key = key();
    let (sender, mut receiver) =
        tokio::sync::watch::channel(Arc::new(std::collections::HashMap::<
            RuntimeProjectWorkspaceKey,
            RuntimeQueryGenerationState,
        >::new()));
    let next_key = key.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        sender.send_replace(Arc::new(std::collections::HashMap::from([(
            next_key,
            RuntimeQueryGenerationState::Failed {
                expected_generation_digest: Arc::from("blake3-256:byte-generation"),
                reason: Arc::from("fixture terminal"),
            },
        )])));
    });
    let observed = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        wait_for_runtime_query_generation(&mut receiver, &key),
    )
    .await
    .expect("first Search waits for the generation terminal")
    .expect("new byte-generation terminal");
    assert!(matches!(
        observed,
        RuntimeQueryGenerationState::Failed { .. }
    ));
}

#[tokio::test]
async fn failed_first_search_does_not_reuse_the_stale_failure_terminal() {
    let key = key();
    let stale = RuntimeQueryGenerationState::Failed {
        expected_generation_digest: Arc::from("blake3-256:stale"),
        reason: Arc::from("stale provider failure"),
    };
    let (sender, mut receiver) = tokio::sync::watch::channel(Arc::new(
        std::collections::HashMap::from([(key.clone(), stale)]),
    ));
    let next_key = key.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        sender.send_replace(Arc::new(std::collections::HashMap::from([(
            next_key,
            RuntimeQueryGenerationState::Failed {
                expected_generation_digest: Arc::from("blake3-256:fresh"),
                reason: Arc::from("fresh byte-generation failure"),
            },
        )])));
    });
    let observed = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        wait_for_runtime_query_generation_change(&mut receiver, &key),
    )
    .await
    .expect("recovery waits for a fresh generation terminal")
    .expect("fresh recovery terminal");
    let RuntimeQueryGenerationState::Failed { reason, .. } = observed else {
        panic!("fixture publishes a replacement failure")
    };
    assert_eq!(reason.as_ref(), "fresh byte-generation failure");
}

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

#[test]
fn response_and_transport_hooks_complete_and_release_the_request_trace() {
    use agent_semantic_client_db::runtime_server_opentelemetry::{
        RuntimeSearchTelemetryIdentity, RuntimeSearchTelemetryIdentityInput,
        RuntimeSearchTelemetryTrace,
    };

    let project_id = ClientProjectId::new("project-trace").expect("project id");
    let workspace_id = ClientWorkspaceIdentity::new("workspace-trace").expect("workspace id");
    let session_id = ClientSessionId::new("session-trace").expect("session id");
    let request_id = ClientRequestId::new("request-trace").expect("request id");
    let key: ClientRequestKey = (
        project_id.clone(),
        workspace_id.clone(),
        session_id.clone(),
        request_id.clone(),
    );
    let identity = RuntimeSearchTelemetryIdentity::new(RuntimeSearchTelemetryIdentityInput {
        session_id: session_id.as_str().to_owned(),
        request_id: request_id.as_str().to_owned(),
        workspace_identity: workspace_id.as_str().to_owned(),
        workspace_snapshot_digest: digest('1'),
        source_snapshot_digest: digest('2'),
        source_generation_digest: digest('3'),
        source_index_digest: digest('4'),
        runtime_artifact_digest: digest('5'),
        runtime_bundle_digest: digest('6'),
        execution_publication_digest: digest('7'),
        provider_catalog_digest: digest('8'),
        language_ids: vec!["rust".into()],
        provider_ids: vec!["asp-rust".into()],
    })
    .expect("trace identity");
    let trace = RuntimeSearchTelemetryTrace::new(identity);
    for phase in ["launcher", "client-frame-encode", "ipc-connect"] {
        trace
            .record_client_phase(phase, 1, 1_000)
            .expect("client phase");
    }
    trace
        .record_server_admission_queue(1, 1_000)
        .expect("admission phase");
    trace
        .record_snapshot_resolve(1, 1_000)
        .expect("snapshot phase");
    trace
        .record_provider_dispatch(1, 1_000)
        .expect("dispatch phase");
    trace
        .record_search_execution(1, 1_000)
        .expect("execution phase");
    trace
        .record_search_projection(1, 1_000)
        .expect("projection phase");
    let traces = std::sync::Mutex::new(std::collections::HashMap::from([(key, trace.clone())]));
    let active = std::sync::atomic::AtomicUsize::new(1);
    let telemetry = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let response = AspClientResponseTelemetry {
        project_id,
        workspace_id,
        session_id,
        request_id,
        outcome: ClientOutcome::Ready,
    };

    record_runtime_response_serialized(&active, &traces, &telemetry.sender, &response, 1);
    record_runtime_terminal_egressed(&active, &traces, &telemetry.sender, &response, 1, true);

    assert_eq!(
        trace
            .observations()
            .iter()
            .map(|observation| observation.stage.as_str())
            .collect::<Vec<_>>(),
        [
            "launcher",
            "client-frame-encode",
            "ipc-connect",
            "server-admission-queue",
            "snapshot-resolve",
            "provider-dispatch",
            "parse-index-query",
            "projection-rank",
            "schema-validate-serialize",
            "terminal-egress",
        ]
    );
    assert_eq!(trace.terminal_count(), 1);
    assert!(traces.lock().expect("trace registry").is_empty());
    assert_eq!(active.load(std::sync::atomic::Ordering::Acquire), 0);

    record_runtime_terminal_egressed(&active, &traces, &telemetry.sender, &response, 1, true);
    assert_eq!(trace.terminal_count(), 1);
}
