use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use super::probe_runtime_server_for_hook;
use crate::runtime_server_control::frame::{read_frame_sync, write_frame_sync};
use crate::runtime_server_control::{
    RuntimeServerControlReceipt, RuntimeServerControlRequest, RuntimeServerHookProbeError,
    RuntimeServerState, prepare_runtime_server_endpoint, prepare_runtime_server_endpoint_in,
    validate_runtime_server_endpoint_for_state_home,
};

async fn fixture_endpoint() -> (tempfile::TempDir, crate::RuntimeServerEndpoint) {
    let state_home = tempfile::Builder::new()
        .prefix("asp-hook-probe-")
        .tempdir_in("/tmp")
        .expect("create Hook probe State Home");
    let endpoint = prepare_runtime_server_endpoint_in(
        state_home.path(),
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        "dev",
        &format!("blake3-256:{}", blake3::hash(b"catalog").to_hex()),
        41,
        "hook-probe-binding",
    )
    .await
    .expect("prepare Hook probe endpoint");
    (state_home, endpoint)
}

fn serve_one_receipt(
    endpoint: &crate::RuntimeServerEndpoint,
    mutate: impl FnOnce(RuntimeServerControlReceipt) -> RuntimeServerControlReceipt + Send + 'static,
) -> std::thread::JoinHandle<()> {
    let listener = UnixListener::bind(&endpoint.socket_path).expect("bind Hook probe listener");
    let endpoint = endpoint.clone();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept Hook probe");
        let requests: Vec<RuntimeServerControlRequest> =
            read_frame_sync(&mut stream).expect("read Hook probe request");
        assert_eq!(requests.len(), 1);
        requests[0]
            .validate_for_endpoint(&endpoint)
            .expect("validate shared Hook probe request");
        let receipt = mutate(RuntimeServerControlReceipt::healthy(
            requests[0].request_id.clone(),
            &endpoint,
            0,
        ));
        write_frame_sync(&mut stream, &[receipt]).expect("write Hook probe receipt");
    })
}

#[tokio::test(flavor = "current_thread")]
async fn healthy_control_frame_is_authoritative() {
    let (_state_home, endpoint) = fixture_endpoint().await;
    let server = serve_one_receipt(&endpoint, |receipt| receipt);
    let receipt = probe_runtime_server_for_hook(&endpoint, "hook-healthy".to_owned())
        .expect("healthy server is live");
    assert_eq!(receipt.state, RuntimeServerState::Healthy);
    server.join().expect("join Hook probe server");
}

#[tokio::test(flavor = "current_thread")]
async fn draining_control_frame_is_live_and_not_stale() {
    let (_state_home, endpoint) = fixture_endpoint().await;
    let server = serve_one_receipt(&endpoint, |mut receipt| {
        receipt.state = RuntimeServerState::Draining;
        receipt
    });
    let receipt = probe_runtime_server_for_hook(&endpoint, "hook-draining".to_owned())
        .expect("draining server still owns the socket");
    assert_eq!(receipt.state, RuntimeServerState::Draining);
    server.join().expect("join Hook probe server");
}

#[tokio::test(flavor = "current_thread")]
async fn starting_control_frame_is_live_and_not_stale() {
    let (_state_home, endpoint) = fixture_endpoint().await;
    let server = serve_one_receipt(&endpoint, |mut receipt| {
        receipt.state = RuntimeServerState::Starting;
        receipt
    });
    let receipt = probe_runtime_server_for_hook(&endpoint, "hook-starting".to_owned())
        .expect("starting server still owns the socket");
    assert_eq!(receipt.state, RuntimeServerState::Starting);
    server.join().expect("join Hook probe server");
}

#[tokio::test(flavor = "current_thread")]
async fn refused_socket_is_the_only_spawnable_stale_class() {
    let (_state_home, endpoint) = fixture_endpoint().await;
    let listener = UnixListener::bind(&endpoint.socket_path).expect("bind stale socket");
    drop(listener);
    let error = probe_runtime_server_for_hook(&endpoint, "hook-refused".to_owned())
        .expect_err("refused endpoint must be stale");
    assert!(matches!(error, RuntimeServerHookProbeError::Stale(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn missing_socket_is_spawnable_stale() {
    let (_state_home, endpoint) = fixture_endpoint().await;
    let error = probe_runtime_server_for_hook(&endpoint, "hook-missing".to_owned())
        .expect_err("missing socket must be stale");
    assert!(matches!(error, RuntimeServerHookProbeError::Stale(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn no_frame_is_bounded_live_transient_and_never_stale() {
    let (_state_home, endpoint) = fixture_endpoint().await;
    let listener = UnixListener::bind(&endpoint.socket_path).expect("bind no-frame listener");
    let server = std::thread::spawn(move || {
        let (_stream, _) = listener.accept().expect("accept no-frame probe");
        std::thread::sleep(Duration::from_millis(150));
    });
    let started = Instant::now();
    let error = probe_runtime_server_for_hook(&endpoint, "hook-no-frame".to_owned())
        .expect_err("missing frame must fail within Hook budget");
    assert!(matches!(
        error,
        RuntimeServerHookProbeError::LiveTransient(_)
    ));
    assert!(started.elapsed() < Duration::from_millis(140));
    server.join().expect("join no-frame server");
}

#[tokio::test(flavor = "current_thread")]
async fn receipt_identity_mismatch_fails_closed() {
    let (_state_home, endpoint) = fixture_endpoint().await;
    let server = serve_one_receipt(&endpoint, |mut receipt| {
        receipt.request_id = "wrong-request".to_owned();
        receipt
    });
    let error = probe_runtime_server_for_hook(&endpoint, "hook-identity".to_owned())
        .expect_err("receipt identity mismatch must fail closed");
    assert!(matches!(error, RuntimeServerHookProbeError::FailClosed(_)));
    server.join().expect("join Hook probe server");
}

#[tokio::test(flavor = "current_thread")]
async fn endpoint_state_home_binding_mismatch_fails_closed() {
    let state_home = tempfile::tempdir().expect("create State Home endpoint owner");
    let endpoint = prepare_runtime_server_endpoint(
        state_home.path(),
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        "dev",
        &format!("blake3-256:{}", blake3::hash(b"catalog").to_hex()),
        42,
        "state-home-binding",
    )
    .await
    .expect("prepare State Home endpoint");
    validate_runtime_server_endpoint_for_state_home(state_home.path(), &endpoint)
        .expect("endpoint must bind its owner State Home");
    let other_state_home = tempfile::tempdir().expect("create other State Home");
    let error = validate_runtime_server_endpoint_for_state_home(other_state_home.path(), &endpoint)
        .expect_err("cross-State-Home endpoint must fail closed");
    assert!(error.contains("State Home or binding identity mismatch"));
}
