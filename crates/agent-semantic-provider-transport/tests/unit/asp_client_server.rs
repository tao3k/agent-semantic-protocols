use bytes::Bytes;
use tokio::{net::TcpListener, sync::watch};

use super::{AspClientServerHttpClient, AspClientServerPeer, AspClientServerSpec};
use crate::{AspClientServerRequest, AspClientServerResponse, serve_asp_client_server};

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
        _ => Ok(AspClientServerResponse {
            status: 404,
            body: Bytes::from_static(br#"{"error":"not-found"}"#),
        }),
    }
}

#[tokio::test]
async fn provider_http_request_deadline_fails_closed_when_the_server_never_responds() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stalled provider fixture");
    let address = listener.local_addr().expect("stalled provider address");
    let stalled_server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("accept provider request");
        std::future::pending::<()>().await;
    });
    let client = AspClientServerHttpClient::new_with_timeout(
        reqwest::Url::parse(&format!("http://{address}/")).expect("provider URL"),
        std::time::Duration::from_millis(25),
    )
    .expect("provider HTTP client");

    let error = client
        .json("GET", "/health", None)
        .await
        .expect_err("stalled provider request must reach a terminal deadline");
    assert!(
        error == "ASP Client Server request deadline exceeded",
        "deadline failure must remain typed: {error}"
    );
    stalled_server.abort();
}

#[tokio::test]
async fn tokio_http_fixture_covers_health_request_and_shutdown_without_external_runtime() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind Tokio HTTP fixture");
    let address = listener.local_addr().expect("Tokio HTTP fixture address");
    let (shutdown, shutdown_reader) = watch::channel(false);
    let server = tokio::spawn(serve_asp_client_server(
        listener,
        shutdown_reader,
        move |request| {
            let shutdown = shutdown.clone();
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

#[test]
fn provider_host_defaults_to_loopback_ephemeral_port() {
    let spec = AspClientServerSpec::new(
        "asp-rust",
        std::env::current_dir().expect("current test directory"),
    );

    assert_eq!(spec.host, "127.0.0.1:0");
}
