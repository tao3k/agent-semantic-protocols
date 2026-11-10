// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use bytes::Bytes;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::watch;

use super::AspClientServerHttpClient;
use super::AspClientServerPeer;
use super::AspClientServerSpec;
use super::utf8_chunks;
use crate::AspClientServerRequest;
use crate::AspClientServerResponse;
use crate::serve_asp_client_server;
use crate::spawn_provider_runtime_peer_actor;

fn serve_request(
    request: AspClientServerRequest,
    shutdown: watch::Sender<bool>,
) -> Result<AspClientServerResponse, String> {
    match request.path.as_str() {
        "/health" => Ok(AspClientServerResponse {
            status: 200,
            body: Bytes::from_static(br#"{"state":"ready"}"#),
        }),
        "/v1/provider-runtime" => Ok(AspClientServerResponse {
            status: 200,
            body: request.body,
        }),
        "/shutdown" => {
            shutdown.send_replace(true);
            Ok(AspClientServerResponse {
                status: 200,
                body: Bytes::from_static(br#"{"state":"draining"}"#),
            })
        }
        "/failure" => Ok(AspClientServerResponse {
            status: 500,
            body: Bytes::from_static(br#"{"error":"projection-failed"}"#),
        }),
        _ => Ok(AspClientServerResponse {
            status: 404,
            body: Bytes::from_static(br#"{"error":"not-found"}"#),
        }),
    }
}

#[tokio::test]
async fn provider_http_lifecycle_deadline_fails_closed_when_the_server_never_responds() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stalled provider fixture");
    let address = listener.local_addr().expect("stalled provider address");
    let stalled_server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("accept provider request");
        std::future::pending::<()>().await;
    });
    let client = AspClientServerHttpClient::new(
        reqwest::Url::parse(&format!("http://{address}/")).expect("provider URL"),
    )
    .expect("provider HTTP client");

    let error = client
        .json_with_deadline("GET", "/health", None, std::time::Duration::from_millis(25))
        .await
        .expect_err("stalled provider request must reach a terminal deadline");
    assert!(
        error == "ASP Client Server lifecycle deadline exceeded",
        "deadline failure must remain typed: {error}"
    );
    stalled_server.abort();
}

#[tokio::test]
async fn provider_http_generation_frame_is_not_cut_by_a_lifecycle_deadline() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind delayed provider fixture");
    let address = listener.local_addr().expect("delayed provider address");
    let (_shutdown, shutdown_reader) = watch::channel(false);
    let server = tokio::spawn(serve_asp_client_server(
        listener,
        shutdown_reader,
        |request| async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(AspClientServerResponse {
                status: 200,
                body: request.body,
            })
        },
    ));
    let client = AspClientServerHttpClient::new(
        reqwest::Url::parse(&format!("http://{address}/")).expect("provider URL"),
    )
    .expect("provider HTTP client");
    let payload = br#"{"operation":"generation-projection"}"#;

    let response = client
        .json("POST", "/v1/provider-runtime", Some(payload))
        .await
        .expect("generation frame remains governed by its task cancellation owner");
    assert_eq!(response, payload);

    server.abort();
}

#[tokio::test]
async fn tokio_http_fixture_covers_health_request_and_shutdown_without_external_runtime() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind Tokio HTTP fixture");
    let address = listener.local_addr().expect("Tokio HTTP fixture address");
    let (shutdown, shutdown_reader) = watch::channel(false);
    let request_shutdown = shutdown.clone();
    let server = tokio::spawn(serve_asp_client_server(
        listener,
        shutdown_reader,
        move |request| {
            let shutdown = request_shutdown.clone();
            async move { serve_request(request, shutdown) }
        },
    ));

    let client = AspClientServerHttpClient::new(
        reqwest::Url::parse(&format!("http://{address}/"))
            .expect("construct Tokio HTTP fixture URL"),
    )
    .expect("construct ASP Client Server client");
    assert_eq!(
        client.json("GET", "/health", None).await.expect("health"),
        br#"{"state":"ready"}"#
    );
    let payload = br#"{"operation":"provider-search"}"#;
    assert_eq!(
        client
            .json("POST", "/v1/provider-runtime", Some(payload))
            .await
            .expect("request"),
        payload
    );
    assert_eq!(
        client
            .json("POST", "/shutdown", Some(b"{}"))
            .await
            .expect("shutdown"),
        br#"{"state":"draining"}"#
    );
    server
        .await
        .expect("join Tokio HTTP fixture")
        .expect("serve Tokio HTTP fixture");
}

#[tokio::test]
async fn non_success_response_preserves_bounded_provider_evidence() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind Tokio HTTP fixture");
    let address = listener.local_addr().expect("Tokio HTTP fixture address");
    let (shutdown, shutdown_reader) = watch::channel(false);
    let request_shutdown = shutdown.clone();
    let server = tokio::spawn(serve_asp_client_server(
        listener,
        shutdown_reader,
        move |request| {
            let shutdown = request_shutdown.clone();
            async move { serve_request(request, shutdown) }
        },
    ));
    let client = AspClientServerHttpClient::new(
        reqwest::Url::parse(&format!("http://{address}/"))
            .expect("construct Tokio HTTP fixture URL"),
    )
    .expect("construct ASP Client Server client");

    let error = client
        .json("POST", "/failure", Some(b"{}"))
        .await
        .expect_err("non-success response must fail closed");
    assert!(error.contains("reasonKind=asp-client-server-http-status"));
    assert!(error.contains("500 Internal Server Error"));
    assert!(error.contains("projection-failed"));

    shutdown.send_replace(true);
    server
        .await
        .expect("join Tokio HTTP fixture")
        .expect("serve Tokio HTTP fixture");
}

#[cfg(unix)]
#[tokio::test]
async fn provider_exit_before_bootstrap_reports_launch_and_bounded_stderr() {
    let mut spec = AspClientServerSpec::new(
        "/bin/sh",
        std::env::current_dir().expect("current test directory"),
    );
    spec.args = vec![
        "-c".to_owned(),
        "printf 'provider-bootstrap-sentinel' >&2; exit 23".to_owned(),
    ];

    let peer = AspClientServerPeer::start(spec)
        .await
        .expect("start early-exit provider fixture");
    let error = peer
        .contract_receipt()
        .await
        .expect_err("early provider exit must fail before Ready publication");

    assert!(error.contains("provider HTTP server exited before bootstrap"));
    assert!(error.contains("status=exit status: 23"));
    assert!(error.contains("program=/bin/sh"));
    assert!(error.contains("provider-bootstrap-sentinel"));
}

#[cfg(unix)]
#[tokio::test]
async fn pending_production_handshake_observes_child_exit_as_typed_failed_terminal() {
    static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    let fixture_id = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let fifo = std::env::temp_dir().join(format!(
        "asp-client-server-child-exit-{}-{fixture_id}.fifo",
        std::process::id()
    ));
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("create child lifecycle FIFO");
    assert!(status.success(), "mkfifo failed: {status}");

    let mut spec = AspClientServerSpec::new(
        "/bin/sh",
        std::env::current_dir().expect("current test directory"),
    );
    spec.args = vec![
        "-c".to_owned(),
        "read _ < \"$1\"; printf 'pending-child-exit' >&2; exit 29".to_owned(),
        "asp-child-fixture".to_owned(),
        fifo.to_string_lossy().into_owned(),
    ];
    let peer = AspClientServerPeer::start(spec)
        .await
        .expect("start pending child fixture");
    let authority = spawn_provider_runtime_peer_actor(8, peer);
    let mut client = authority.client();

    let mut release = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&fifo)
        .await
        .expect("open child lifecycle FIFO");
    release
        .write_all(b"exit\n")
        .await
        .expect("release pending child");
    drop(release);

    let error = client
        .wait_ready()
        .await
        .expect_err("child exit must fail readiness");
    assert!(
        error.contains("reasonKind=provider-runtime-peer-eof")
            || (error.contains("provider HTTP server exited before bootstrap")
                && error.contains("status=exit status: 29")),
        "unexpected child terminal: {error}"
    );
    authority.shutdown().await.expect("join failed actor");
    std::fs::remove_file(&fifo).expect("remove child lifecycle FIFO");
}

#[test]
fn provider_request_stream_chunks_preserve_utf8_boundaries_and_content() {
    let source = "a界".repeat(100_000);
    let chunks = utf8_chunks(&source, 128 * 1024);

    assert!(chunks.len() > 1);
    assert!(chunks.iter().all(|chunk| chunk.len() <= 128 * 1024));
    assert_eq!(chunks.concat(), source);
}

#[test]
fn provider_host_defaults_to_loopback_ephemeral_port() {
    let spec = AspClientServerSpec::new(
        "asp-rust",
        std::env::current_dir().expect("current test directory"),
    );

    assert_eq!(spec.host, "127.0.0.1:0");
}
