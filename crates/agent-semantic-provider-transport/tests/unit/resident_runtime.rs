use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;
use futures_util::StreamExt;

use super::*;
use crate::{ProviderRuntimeContractOperation, ProviderRuntimeContractTransport};

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn receipt() -> ProviderRuntimeContractReceipt {
    ProviderRuntimeContractReceipt::new(
        "rs-harness",
        "rust",
        digest('a'),
        digest('b'),
        ProviderRuntimeContractTransport::RuntimeIpcV1,
        vec![ProviderRuntimeContractOperation {
            operation: "provider-search".to_owned(),
            request_schema_id: "agent.semantic-protocols.runtime-provider-search-request"
                .to_owned(),
            response_schema_id: "agent.semantic-protocols.runtime-provider-search-receipt"
                .to_owned(),
        }],
    )
    .expect("runtime contract receipt")
}

#[tokio::test]
async fn starting_runtime_fails_requests_immediately_and_shutdown_cancels_handshake() {
    let (_release, blocked) = tokio::sync::oneshot::channel::<()>();
    let authority = spawn_provider_runtime_actor(
        8,
        move || async move {
            blocked
                .await
                .map_err(|_| "handshake cancelled".to_owned())?;
            Ok(receipt())
        },
        |_operation, payload| async move { Ok(payload) },
    );
    let client = authority.client();
    let mut states = client.states();
    assert_eq!(
        states.next().await,
        Some(ProviderRuntimeActorState::Starting)
    );
    assert_eq!(client.current(), ProviderRuntimeActorState::Starting);
    tokio::task::yield_now().await;
    assert_eq!(
        states.next().await,
        Some(ProviderRuntimeActorState::Warming)
    );
    assert_eq!(client.current(), ProviderRuntimeActorState::Warming);
    assert_eq!(
        client
            .request("provider-search", Bytes::from_static(b"request"))
            .await
            .expect_err("Warming must fail closed"),
        "provider-runtime-not-ready: state=warming"
    );
    authority.shutdown().await.expect("shutdown Warming actor");
    assert_eq!(client.current(), ProviderRuntimeActorState::Stopped);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_hundred_fifty_six_concurrent_requests_share_one_resident_actor() {
    let handshakes = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let handshake_count = Arc::clone(&handshakes);
    let call_count = Arc::clone(&calls);
    let authority = spawn_provider_runtime_actor(
        64,
        move || async move {
            handshake_count.fetch_add(1, Ordering::AcqRel);
            Ok(receipt())
        },
        move |_operation, payload| {
            let call_count = Arc::clone(&call_count);
            async move {
                call_count.fetch_add(1, Ordering::AcqRel);
                Ok(payload)
            }
        },
    );
    let mut ready = authority.client();
    ready.wait_ready().await.expect("resident actor Ready");

    let mut tasks = tokio::task::JoinSet::new();
    for index in 0_u16..256 {
        let client = ready.clone();
        tasks.spawn(async move {
            let payload = Bytes::copy_from_slice(&index.to_le_bytes());
            let response = client
                .request("provider-search", payload.clone())
                .await
                .expect("resident request");
            assert_eq!(response, payload);
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.expect("request task");
    }

    assert_eq!(handshakes.load(Ordering::Acquire), 1);
    assert_eq!(calls.load(Ordering::Acquire), 256);
    authority.shutdown().await.expect("shutdown Ready actor");
    assert_eq!(ready.current(), ProviderRuntimeActorState::Stopped);
    assert!(
        ready
            .request("provider-search", Bytes::new())
            .await
            .expect_err("closed actor must fail")
            .contains("state=stopped")
    );
}

#[tokio::test]
async fn failed_handshake_is_terminal_and_never_runs_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let call_count = Arc::clone(&calls);
    let authority = spawn_provider_runtime_actor(
        8,
        || async { Err("typed handshake rejection".to_owned()) },
        move |_operation, payload| {
            let call_count = Arc::clone(&call_count);
            async move {
                call_count.fetch_add(1, Ordering::AcqRel);
                Ok(payload)
            }
        },
    );
    let mut client = authority.client();
    assert_eq!(
        client.wait_ready().await.expect_err("handshake must fail"),
        "typed handshake rejection"
    );
    assert_eq!(calls.load(Ordering::Acquire), 0);
    authority.shutdown().await.expect("join failed actor");
}
