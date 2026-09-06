// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::{QueryConnectionOutcome, record_query_connection_terminal, serve_query};
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn eof_before_frame_completes_liveness_probe_accounting() {
    let (client, server) = tokio::net::UnixStream::pair().expect("create query socket pair");
    drop(client);
    let scope =
        crate::runtime_server_runtime::RuntimeServerTaskScope::new("telemetry-query-probe-test");
    let permit = scope
        .permit("runtime-server-telemetry-query-connection")
        .expect("admit probe connection");
    let result = serve_query(
        server,
        std::sync::Arc::new(
            crate::runtime_server_opentelemetry::live_store::RuntimePerformanceLiveStore::default(),
        ),
    )
    .await;
    assert_eq!(result, Ok(QueryConnectionOutcome::LivenessProbe));
    record_query_connection_terminal(permit, &result);

    let receipt = scope.finish(0).expect("probe accounting must drain");
    assert_eq!(receipt.started, 1);
    assert_eq!(receipt.completed, 1);
    assert_eq!(receipt.failed, 0);
}

#[tokio::test]
async fn non_empty_malformed_frame_remains_failed_accounting() {
    let (mut client, server) = tokio::net::UnixStream::pair().expect("create query socket pair");
    client
        .write_all(b"{malformed}\n")
        .await
        .expect("write malformed query frame");
    client.shutdown().await.expect("finish malformed frame");
    let scope = crate::runtime_server_runtime::RuntimeServerTaskScope::new(
        "telemetry-query-malformed-test",
    );
    let permit = scope
        .permit("runtime-server-telemetry-query-connection")
        .expect("admit malformed connection");
    let result = serve_query(
        server,
        std::sync::Arc::new(
            crate::runtime_server_opentelemetry::live_store::RuntimePerformanceLiveStore::default(),
        ),
    )
    .await;
    assert!(
        result
            .as_ref()
            .is_err_and(|error| error.contains("failed to decode Runtime Server telemetry query"))
    );
    record_query_connection_terminal(permit, &result);

    let receipt = scope
        .finish(0)
        .expect("failed request accounting must still drain");
    assert_eq!(receipt.started, 1);
    assert_eq!(receipt.completed, 0);
    assert_eq!(receipt.failed, 1);
}

#[tokio::test]
async fn non_empty_partial_frame_eof_is_one_typed_terminal_failure() {
    let (mut client, server) = tokio::net::UnixStream::pair().expect("create query socket pair");
    client
        .write_all(br#"{\"requestId\":\"partial"#)
        .await
        .expect("write partial query frame");
    client.shutdown().await.expect("terminate partial frame");

    let scope = crate::runtime_server_runtime::RuntimeServerTaskScope::new(
        "telemetry-query-partial-frame-test",
    );
    let permit = scope
        .permit("runtime-server-telemetry-query-connection")
        .expect("admit partial-frame connection");
    let result = serve_query(
        server,
        std::sync::Arc::new(
            crate::runtime_server_opentelemetry::live_store::RuntimePerformanceLiveStore::default(),
        ),
    )
    .await;

    let error = result.expect_err("a non-empty partial frame must not become a liveness probe");
    let terminal: serde_json::Value =
        serde_json::from_str(&error).expect("partial-frame EOF must be a typed terminal");
    assert_eq!(
        terminal["schemaId"],
        "agent.semantic-protocols.client.frame",
    );
    assert_eq!(terminal["schemaVersion"], "1");
    assert_eq!(terminal["reasonKind"], "frame-eof");

    let terminal_result = Err(error);
    record_query_connection_terminal(permit, &terminal_result);
    let receipt = scope
        .finish(0)
        .expect("partial-frame terminal accounting must drain");
    assert_eq!(receipt.started, 1);
    assert_eq!(receipt.completed, 0);
    assert_eq!(receipt.failed, 1);
}
