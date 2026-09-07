// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;

use super::ProviderRuntimeActorState;
use super::ProviderRuntimeContractReceipt;
use super::ProviderRuntimePeer;
use super::spawn_in_process_provider_runtime_actor;
use super::spawn_provider_runtime_peer_actor;

fn ready_receipt() -> ProviderRuntimeContractReceipt {
    ProviderRuntimeContractReceipt::new(
        "asp-rust",
        "rust",
        format!("blake3-256:{}", "a".repeat(64)),
        format!("blake3-256:{}", "b".repeat(64)),
        crate::runtime_contract::ProviderRuntimeContractTransport::RuntimeIpc,
        vec![crate::runtime_contract::ProviderRuntimeContractOperation {
            operation: "owner-items".to_owned(),
            request_schema: agent_semantic_provider_protocol::ProviderSchemaReference {
                schema_id: "agent.semantic-protocols.asp-client-server-request".to_owned(),
                schema_version: "1".to_owned(),
            },
            response_schema: agent_semantic_provider_protocol::ProviderSchemaReference {
                schema_id: "agent.semantic-protocols.asp-client-server-response".to_owned(),
                schema_version: "1".to_owned(),
            },
        }],
    )
    .expect("runtime contract")
}

#[tokio::test]
async fn actor_executes_admitted_requests_concurrently() {
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let authority = spawn_in_process_provider_runtime_actor(8, || async { Ok(ready_receipt()) }, {
        let barrier = std::sync::Arc::clone(&barrier);
        move |_operation, payload| {
            let barrier = std::sync::Arc::clone(&barrier);
            async move {
                barrier.wait().await;
                Ok(payload)
            }
        }
    });
    let mut client = authority.client();
    client.wait_ready().await.expect("ready actor");
    let first = client
        .begin_request("owner-items", Bytes::from_static(b"first"))
        .await
        .expect("first request");
    let second = client
        .begin_request("owner-items", Bytes::from_static(b"second"))
        .await
        .expect("second request");
    let (first, second) = tokio::time::timeout(std::time::Duration::from_millis(100), async {
        tokio::join!(first, second)
    })
    .await
    .expect("requests must not serialize");
    assert_eq!(first.expect("first response"), Bytes::from_static(b"first"));
    assert_eq!(
        second.expect("second response"),
        Bytes::from_static(b"second")
    );
    authority.shutdown().await.expect("shutdown actor");
}

#[tokio::test]
async fn peer_actor_concurrency_and_idle_drain_share_the_tokio_authority() {
    struct ConcurrentPeer {
        barrier: std::sync::Arc<tokio::sync::Barrier>,
        stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
        lifecycle: tokio::sync::watch::Receiver<bool>,
        lifecycle_writer: tokio::sync::watch::Sender<bool>,
    }

    impl ProviderRuntimePeer for ConcurrentPeer {
        fn handshake(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<ProviderRuntimeContractReceipt, String>> + Send + '_>>
        {
            Box::pin(async { Ok(ready_receipt()) })
        }

        fn request(
            &self,
            _operation: String,
            payload: Bytes,
        ) -> Pin<Box<dyn Future<Output = Result<Bytes, String>> + Send + '_>> {
            let barrier = std::sync::Arc::clone(&self.barrier);
            Box::pin(async move {
                barrier.wait().await;
                Ok(payload)
            })
        }

        fn shutdown(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
            Box::pin(async {
                self.stopped
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                self.lifecycle_writer.send_replace(true);
                Ok(())
            })
        }

        fn wait_terminated(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
            Box::pin(async move {
                let mut lifecycle = self.lifecycle.clone();
                while !*lifecycle.borrow() {
                    lifecycle
                        .changed()
                        .await
                        .map_err(|_| "concurrent peer lifecycle publication closed".to_owned())?;
                }
                Ok(())
            })
        }
    }

    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (lifecycle_writer, lifecycle) = tokio::sync::watch::channel(false);
    let authority = spawn_provider_runtime_peer_actor(
        8,
        ConcurrentPeer {
            barrier,
            stopped: std::sync::Arc::clone(&stopped),
            lifecycle,
            lifecycle_writer,
        },
    );
    let mut client = authority.client();
    client.wait_ready().await.expect("ready peer actor");
    let first = client
        .begin_request("owner-items", Bytes::from_static(b"first"))
        .await
        .expect("first peer request");
    let second = client
        .begin_request("owner-items", Bytes::from_static(b"second"))
        .await
        .expect("second peer request");
    let (first, second) = tokio::time::timeout(std::time::Duration::from_millis(100), async {
        tokio::join!(first, second)
    })
    .await
    .expect("peer requests must not serialize");
    assert_eq!(first.expect("first response"), Bytes::from_static(b"first"));
    assert_eq!(
        second.expect("second response"),
        Bytes::from_static(b"second")
    );
    authority.drain().await.expect("idle drain peer actor");
    assert!(stopped.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(client.current(), ProviderRuntimeActorState::Stopped);
}

#[tokio::test]
async fn dropping_request_cancels_handler_and_releases_actor_lease() {
    struct ActiveGuard(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for ActiveGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let authority = spawn_in_process_provider_runtime_actor(8, || async { Ok(ready_receipt()) }, {
        let active = std::sync::Arc::clone(&active);
        move |_operation, _payload| {
            let active = std::sync::Arc::clone(&active);
            async move {
                active.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _guard = ActiveGuard(active);
                std::future::pending::<Result<Bytes, String>>().await
            }
        }
    });
    let mut client = authority.client();
    client.wait_ready().await.expect("ready actor");
    let request = client
        .begin_request("owner-items", Bytes::new())
        .await
        .expect("admitted request");
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while active.load(std::sync::atomic::Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("handler started");
    drop(request);
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while active.load(std::sync::atomic::Ordering::SeqCst) != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled handler released its lease");
    authority.shutdown().await.expect("shutdown actor");
}

#[tokio::test]
async fn drain_rejects_new_work_and_waits_for_existing_request_lease() {
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    let authority = spawn_in_process_provider_runtime_actor(8, || async { Ok(ready_receipt()) }, {
        let release = std::sync::Arc::clone(&release);
        move |_operation, payload| {
            let release = std::sync::Arc::clone(&release);
            async move {
                release.notified().await;
                Ok(payload)
            }
        }
    });
    let mut client = authority.client();
    client.wait_ready().await.expect("ready actor");
    let admitted = client
        .begin_request("owner-items", Bytes::from_static(b"admitted"))
        .await
        .expect("admitted request");
    let drain = tokio::spawn(authority.drain());
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while client.current() != ProviderRuntimeActorState::Draining {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actor entered draining");
    let rejected = match client.begin_request("owner-items", Bytes::new()).await {
        Ok(_) => panic!("draining actor accepted new work"),
        Err(error) => error,
    };
    assert_eq!(rejected, "asp-client-server-not-ready: state=draining");
    release.notify_waiters();
    assert_eq!(
        admitted.await.expect("admitted response"),
        Bytes::from_static(b"admitted")
    );
    drain.await.expect("drain task").expect("drain receipt");
    assert_eq!(client.current(), ProviderRuntimeActorState::Stopped);
}
