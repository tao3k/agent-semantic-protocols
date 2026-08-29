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
        ProviderRuntimeContractTransport::RuntimeIpc,
        vec![ProviderRuntimeContractOperation {
            operation: "provider-search".to_owned(),
            request_schema: agent_semantic_provider_protocol::ProviderSchemaReference {
                schema_id: "agent.semantic-protocols.runtime-provider-search-request".to_owned(),
                schema_version: "1".to_owned(),
            },
            response_schema: agent_semantic_provider_protocol::ProviderSchemaReference {
                schema_id: "agent.semantic-protocols.runtime-provider-search-receipt".to_owned(),
                schema_version: "1".to_owned(),
            },
        }],
    )
    .expect("runtime contract receipt")
}

#[test]
fn actor_terminal_guard_preserves_the_first_typed_failure() {
    let (state_writer, state) = watch::channel(ProviderRuntimeActorState::Starting);
    publish_actor_failure(
        &state_writer,
        "state=provider-runtime-terminal reasonKind=first-failure phase=test".to_owned(),
    );
    publish_actor_failure(
        &state_writer,
        "state=provider-runtime-terminal reasonKind=second-failure phase=test".to_owned(),
    );

    assert_eq!(
        state.borrow().clone(),
        ProviderRuntimeActorState::Failed(
            "state=provider-runtime-terminal reasonKind=first-failure phase=test".to_owned()
        )
    );
}

#[tokio::test]
async fn starting_runtime_fails_requests_immediately_and_shutdown_cancels_handshake() {
    let (_release, blocked) = tokio::sync::oneshot::channel::<()>();
    let authority = spawn_in_process_provider_runtime_actor(
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
    let mut states = client.lifecycle_states(receipt());
    assert_eq!(
        states
            .next()
            .await
            .expect("Starting lifecycle publication")
            .expect("valid Starting lifecycle")
            .state,
        crate::AspClientServerLifecycleState::Starting,
    );
    assert_eq!(client.current(), ProviderRuntimeActorState::Starting);
    tokio::task::yield_now().await;
    assert_eq!(
        states
            .next()
            .await
            .expect("Warming lifecycle publication")
            .expect("valid Warming lifecycle")
            .state,
        crate::AspClientServerLifecycleState::Warming,
    );
    assert_eq!(client.current(), ProviderRuntimeActorState::Warming);
    assert_eq!(
        client
            .request("provider-search", Bytes::from_static(b"request"))
            .await
            .expect_err("Warming must fail closed"),
        "asp-client-server-not-ready: state=warming"
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
    let authority = spawn_in_process_provider_runtime_actor(
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
    let authority = spawn_in_process_provider_runtime_actor(
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

#[tokio::test]
async fn actor_panic_is_published_as_typed_failed_terminal() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let handshake_entered = Arc::clone(&entered);
    let authority = spawn_in_process_provider_runtime_actor(
        8,
        move || async move {
            handshake_entered.notify_one();
            panic!("deterministic actor panic");
            #[allow(unreachable_code)]
            Ok(receipt())
        },
        |_operation, payload| async move { Ok(payload) },
    );
    let mut client = authority.client();
    entered.notified().await;
    let error = client
        .wait_ready()
        .await
        .expect_err("actor panic must fail readiness");
    assert!(
        error.contains("reasonKind=provider-runtime-actor-task-panicked"),
        "unexpected terminal: {error}"
    );
    authority.join().await.expect("join actor supervisor");
}

#[tokio::test]
async fn actor_abort_is_published_as_typed_failed_terminal() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let handshake_entered = Arc::clone(&entered);
    let authority = spawn_in_process_provider_runtime_actor(
        8,
        move || async move {
            handshake_entered.notify_one();
            std::future::pending::<Result<ProviderRuntimeContractReceipt, String>>().await
        },
        |_operation, payload| async move { Ok(payload) },
    );
    let mut client = authority.client();
    entered.notified().await;
    authority.abort_actor();
    let error = client
        .wait_ready()
        .await
        .expect_err("actor abort must fail readiness");
    assert!(
        error.contains("reasonKind=provider-runtime-actor-task-aborted"),
        "unexpected terminal: {error}"
    );
    authority.join().await.expect("join actor supervisor");
}

struct PendingPeer {
    handshake_entered: Arc<tokio::sync::Notify>,
    terminated: Arc<tokio::sync::Notify>,
    stopped: Arc<std::sync::atomic::AtomicBool>,
}

impl ProviderRuntimePeer for PendingPeer {
    fn handshake(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_>>
    {
        Box::pin(async {
            self.handshake_entered.notify_one();
            std::future::pending().await
        })
    }

    fn request(
        &self,
        _operation: String,
        _payload: Bytes,
    ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>> {
        Box::pin(std::future::pending())
    }

    fn shutdown(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async {
            self.stopped.store(true, Ordering::Release);
            Ok(())
        })
    }

    fn wait_terminated(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async {
            self.terminated.notified().await;
            Ok(())
        })
    }
}

#[tokio::test]
async fn pending_peer_handshake_lifecycle_cancellation_is_terminal_without_deadline() {
    let handshake_entered = Arc::new(tokio::sync::Notify::new());
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let authority = spawn_provider_runtime_peer_actor(
        8,
        PendingPeer {
            handshake_entered: Arc::clone(&handshake_entered),
            terminated: Arc::new(tokio::sync::Notify::new()),
            stopped: Arc::clone(&stopped),
        },
    );
    let mut client = authority.client();
    handshake_entered.notified().await;
    let shutdown = tokio::spawn(authority.shutdown());
    assert_eq!(
        client
            .wait_ready()
            .await
            .expect_err("startup cancellation must fail readiness"),
        "state=provider-runtime-terminal reasonKind=provider-runtime-startup-cancelled"
    );
    shutdown
        .await
        .expect("shutdown task")
        .expect("shutdown authority");
    assert!(stopped.load(Ordering::Acquire));
}

#[tokio::test]
async fn pending_peer_handshake_peer_eof_is_terminal_without_deadline() {
    let handshake_entered = Arc::new(tokio::sync::Notify::new());
    let terminated = Arc::new(tokio::sync::Notify::new());
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let authority = spawn_provider_runtime_peer_actor(
        8,
        PendingPeer {
            handshake_entered: Arc::clone(&handshake_entered),
            terminated: Arc::clone(&terminated),
            stopped: Arc::clone(&stopped),
        },
    );
    let mut client = authority.client();
    handshake_entered.notified().await;
    terminated.notify_one();
    let error = client
        .wait_ready()
        .await
        .expect_err("provider without bootstrap must fail closed");

    assert_eq!(
        error,
        "state=provider-runtime-terminal reasonKind=provider-runtime-peer-eof"
    );
    authority.shutdown().await.expect("join failed actor");
    assert!(stopped.load(Ordering::Acquire));
}
