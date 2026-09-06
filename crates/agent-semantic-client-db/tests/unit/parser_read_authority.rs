// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::CommittedParserGeneration;
use super::LoadedParserGeneration;
use super::ParserReadAuthority;
use super::ParserReadAuthorityRegistry;
use super::ParserReadContext;
use super::ParserReadCounters;
use super::ParserReadRequest;
use super::ParserReadRoute;
use super::ParserReadState;
use super::RuntimeEndpointState;

fn request() -> ParserReadRequest {
    ParserReadRequest::new(generation()).expect("validated parser read request")
}
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

fn context() -> ParserReadContext {
    ParserReadContext {
        project_id: "repo-a".to_owned(),
        workspace_id: "workspace-a".to_owned(),
        canonical_workspace_root: "/workspace/a".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        provider_manifest_digest: format!("sha256:{}", "a".repeat(64)),
    }
}

fn generation() -> CommittedParserGeneration {
    CommittedParserGeneration {
        context: context(),
        generation_epoch: 1,
        generation_digest: format!("blake3-256:{}", "b".repeat(64)),
        source_root_digest: format!("blake3-256:{}", "c".repeat(64)),
    }
}

#[test]
fn valid_generation_is_readable_for_every_runtime_state() {
    for runtime_state in [
        RuntimeEndpointState::Healthy,
        RuntimeEndpointState::Stopped,
        RuntimeEndpointState::Unavailable,
    ] {
        let authority = ParserReadAuthority::ready(
            generation(),
            runtime_state,
            true,
            0,
            ParserReadCounters::default(),
        )
        .expect("valid read authority");
        assert_eq!(authority.state, ParserReadState::Ready);
        assert_eq!(authority.route, ParserReadRoute::ResidentMemory);
        authority.validate().expect("validated authority");
    }
}

#[test]
fn process_cold_valid_generation_uses_durable_mmap() {
    let authority = ParserReadAuthority::ready(
        generation(),
        RuntimeEndpointState::Stopped,
        false,
        0,
        ParserReadCounters {
            mapped_bytes: 4096,
            ..ParserReadCounters::default()
        },
    )
    .expect("durable mmap authority");
    assert_eq!(authority.route, ParserReadRoute::DurableMmap);
}

#[test]
fn read_authority_rejects_query_time_io_and_mutation() {
    for counters in [
        ParserReadCounters {
            database_opens: 1,
            ..ParserReadCounters::default()
        },
        ParserReadCounters {
            provider_spawns: 1,
            ..ParserReadCounters::default()
        },
        ParserReadCounters {
            source_file_reads: 1,
            ..ParserReadCounters::default()
        },
        ParserReadCounters {
            activation_refreshes: 1,
            ..ParserReadCounters::default()
        },
        ParserReadCounters {
            writes: 1,
            ..ParserReadCounters::default()
        },
    ] {
        assert!(
            ParserReadAuthority::ready(
                generation(),
                RuntimeEndpointState::Unavailable,
                true,
                0,
                counters,
            )
            .is_err()
        );
    }
}

#[test]
fn non_ready_generation_never_exposes_a_read_route() {
    for state in [
        ParserReadState::GenerationRequired,
        ParserReadState::GenerationStale,
        ParserReadState::GenerationCorrupt,
    ] {
        let authority = ParserReadAuthority::unavailable(
            context(),
            Some(&generation()),
            RuntimeEndpointState::Stopped,
            state,
            0,
        )
        .expect("typed unavailable authority");
        assert_eq!(authority.route, ParserReadRoute::None);
        authority.validate().expect("validated unavailable state");
    }
}

#[test]
fn generation_required_does_not_forge_generation_digests() {
    let authority = ParserReadAuthority::unavailable(
        context(),
        None,
        RuntimeEndpointState::Unavailable,
        ParserReadState::GenerationRequired,
        0,
    )
    .expect("typed missing generation");
    assert_eq!(authority.route, ParserReadRoute::None);
    assert_eq!(authority.generation_digest, None);
    assert_eq!(authority.source_root_digest, None);
    assert_eq!(authority.generation_epoch, None);
    authority.validate().expect("validated missing generation");
    let wire = serde_json::to_value(authority).expect("parser authority wire shape");
    assert_eq!(wire["schemaVersion"], "1");
    assert_eq!(wire["workspaceId"], "workspace-a");
    assert_eq!(wire["providerId"], "asp-rust");
    assert_eq!(wire["runtimeEndpointState"], "unavailable");
    assert_eq!(wire["elapsedMicros"], 0);
    assert_eq!(wire["route"], "none");
    assert!(wire.get("generationDigest").is_none());
    assert!(wire.get("sourceRootDigest").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_readers_load_one_immutable_generation_once() {
    let registry = ParserReadAuthorityRegistry::new();
    let loads = Arc::new(AtomicU64::new(0));
    let mut readers = tokio::task::JoinSet::new();
    for _ in 0..256 {
        let registry = registry.clone();
        let loads = loads.clone();
        readers.spawn(async move {
            registry
                .acquire(
                    request(),
                    RuntimeEndpointState::Stopped,
                    move || async move {
                        loads.fetch_add(1, Ordering::Relaxed);
                        Ok(LoadedParserGeneration {
                            generation: generation(),
                            checkpoint_bytes: Arc::from([1_u8, 2, 3]),
                            counters: ParserReadCounters {
                                mapped_bytes: 3,
                                ..ParserReadCounters::default()
                            },
                        })
                    },
                )
                .await
                .expect("shared immutable generation")
        });
    }

    let mut leases = 0;
    let mut durable_routes = 0;
    let mut mapped_bytes = 0;
    while let Some(result) = readers.join_next().await {
        let lease = result.expect("reader task");
        assert_eq!(lease.authority.state, ParserReadState::Ready);
        assert_eq!(lease.generation.checkpoint_bytes.as_ref(), [1_u8, 2, 3]);
        if lease.authority.route == ParserReadRoute::DurableMmap {
            durable_routes += 1;
        }
        mapped_bytes += lease.authority.counters.mapped_bytes;
        leases += 1;
    }
    assert_eq!(leases, 256);
    assert_eq!(loads.load(Ordering::Relaxed), 1);
    assert_eq!(durable_routes, 1);
    assert_eq!(mapped_bytes, 3);
    assert_eq!(registry.resident_generation_count().await, 1);
}

#[tokio::test]
async fn failed_load_does_not_poison_the_generation_cell() {
    let registry = ParserReadAuthorityRegistry::new();
    let first = registry
        .acquire(request(), RuntimeEndpointState::Unavailable, || async {
            Err("synthetic load failure".to_owned())
        })
        .await;
    assert!(first.is_err());

    let retry = registry
        .acquire(request(), RuntimeEndpointState::Stopped, || async {
            Ok(LoadedParserGeneration {
                generation: generation(),
                checkpoint_bytes: Arc::from([9_u8]),
                counters: ParserReadCounters {
                    mapped_bytes: 1,
                    ..ParserReadCounters::default()
                },
            })
        })
        .await
        .expect("retry after failed load");
    assert_eq!(retry.authority.route, ParserReadRoute::DurableMmap);
    assert_eq!(registry.resident_generation_count().await, 1);
}

#[tokio::test]
async fn generation_identity_mismatch_never_enters_resident_memory() {
    let registry = ParserReadAuthorityRegistry::new();
    let mut wrong = generation();
    wrong.source_root_digest = format!("blake3-256:{}", "d".repeat(64));
    let result = registry
        .acquire(request(), RuntimeEndpointState::Healthy, || async {
            Ok(LoadedParserGeneration {
                generation: wrong,
                checkpoint_bytes: Arc::from([1_u8]),
                counters: ParserReadCounters::default(),
            })
        })
        .await;
    assert!(result.is_err());
    assert_eq!(registry.resident_generation_count().await, 0);
}

#[tokio::test]
async fn cancelled_loader_releases_the_single_flight_cell_for_retry() {
    let registry = ParserReadAuthorityRegistry::new();
    let (started_sender, started_receiver) = tokio::sync::oneshot::channel();
    let (_finish_sender, finish_receiver) = tokio::sync::oneshot::channel::<()>();
    let cancelled_registry = registry.clone();
    let cancelled = tokio::spawn(async move {
        cancelled_registry
            .acquire(request(), RuntimeEndpointState::Stopped, || async move {
                let _ = started_sender.send(());
                let _ = finish_receiver.await;
                Err("cancelled loader continued after owner cancellation".to_owned())
            })
            .await
    });
    started_receiver.await.expect("loader started");
    cancelled.abort();
    assert!(
        cancelled
            .await
            .expect_err("cancelled loader task")
            .is_cancelled()
    );

    let retry = registry
        .acquire(request(), RuntimeEndpointState::Stopped, || async {
            Ok(LoadedParserGeneration {
                generation: generation(),
                checkpoint_bytes: Arc::from([7_u8]),
                counters: ParserReadCounters {
                    mapped_bytes: 1,
                    ..ParserReadCounters::default()
                },
            })
        })
        .await
        .expect("retry after loader cancellation");
    assert_eq!(retry.authority.route, ParserReadRoute::DurableMmap);
    assert_eq!(registry.resident_generation_count().await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn warm_read_authority_acquisition_is_zero_reload_and_sub_millisecond_at_p99() {
    let registry = ParserReadAuthorityRegistry::new();
    registry
        .acquire(request(), RuntimeEndpointState::Stopped, || async {
            Ok(LoadedParserGeneration {
                generation: generation(),
                checkpoint_bytes: Arc::from([1_u8, 2, 3]),
                counters: ParserReadCounters {
                    mapped_bytes: 3,
                    ..ParserReadCounters::default()
                },
            })
        })
        .await
        .expect("cold load");

    let warm_request = request();
    let reloads = Arc::new(AtomicU64::new(0));
    let mut samples = Vec::with_capacity(2048);
    for _ in 0..2048 {
        let reloads = reloads.clone();
        let started = tokio::time::Instant::now();
        let lease = registry
            .acquire(
                warm_request.clone(),
                RuntimeEndpointState::Unavailable,
                move || async move {
                    reloads.fetch_add(1, Ordering::Relaxed);
                    Err("warm authority attempted to reload".to_owned())
                },
            )
            .await
            .expect("warm read authority");
        samples.push(started.elapsed().as_nanos());
        assert_eq!(lease.authority.route, ParserReadRoute::ResidentMemory);
        assert_eq!(lease.authority.counters.mapped_bytes, 0);
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99).div_ceil(100) - 1];
    eprintln!(
        "parser-read-authority-warm samples={} p99Nanos={} reloads={}",
        samples.len(),
        p99,
        reloads.load(Ordering::Relaxed)
    );
    assert_eq!(reloads.load(Ordering::Relaxed), 0);
    assert!(
        p99 < 1_000_000,
        "warm parser authority p99 exceeded 1ms: {p99}ns"
    );
}

#[tokio::test]
async fn newer_epoch_replaces_registry_authority_without_invalidating_old_reader_lease() {
    let registry = ParserReadAuthorityRegistry::new();
    let old = registry
        .acquire(request(), RuntimeEndpointState::Stopped, || async {
            Ok(LoadedParserGeneration {
                generation: generation(),
                checkpoint_bytes: Arc::from([1_u8]),
                counters: ParserReadCounters {
                    mapped_bytes: 1,
                    ..ParserReadCounters::default()
                },
            })
        })
        .await
        .expect("old generation lease");

    let mut next_generation = generation();
    next_generation.generation_epoch = 2;
    next_generation.generation_digest = format!("blake3-256:{}", "d".repeat(64));
    next_generation.source_root_digest = format!("blake3-256:{}", "e".repeat(64));
    let loaded_next = next_generation.clone();
    let new = registry
        .acquire(
            ParserReadRequest::new(next_generation).expect("next parser generation request"),
            RuntimeEndpointState::Unavailable,
            move || async move {
                Ok(LoadedParserGeneration {
                    generation: loaded_next,
                    checkpoint_bytes: Arc::from([2_u8]),
                    counters: ParserReadCounters {
                        mapped_bytes: 1,
                        ..ParserReadCounters::default()
                    },
                })
            },
        )
        .await
        .expect("new generation lease");

    assert_eq!(registry.resident_generation_count().await, 1);
    assert_eq!(old.generation.checkpoint_bytes.as_ref(), [1_u8]);
    assert_eq!(new.generation.checkpoint_bytes.as_ref(), [2_u8]);
    assert_eq!(new.authority.generation_epoch, Some(2));
}

#[tokio::test]
async fn stale_or_same_epoch_drift_cannot_replace_resident_generation() {
    let registry = ParserReadAuthorityRegistry::new();
    let mut current = generation();
    current.generation_epoch = 2;
    let loaded_current = current.clone();
    registry
        .acquire(
            ParserReadRequest::new(current.clone()).expect("current parser generation request"),
            RuntimeEndpointState::Healthy,
            move || async move {
                Ok(LoadedParserGeneration {
                    generation: loaded_current,
                    checkpoint_bytes: Arc::from([2_u8]),
                    counters: ParserReadCounters::default(),
                })
            },
        )
        .await
        .expect("current generation");

    let attempted_loads = Arc::new(AtomicU64::new(0));
    let stale_loads = attempted_loads.clone();
    let stale = registry
        .acquire(
            request(),
            RuntimeEndpointState::Stopped,
            move || async move {
                stale_loads.fetch_add(1, Ordering::Relaxed);
                Err("stale loader must not execute".to_owned())
            },
        )
        .await;
    assert!(stale.is_err());

    let mut drift = current;
    drift.source_root_digest = format!("blake3-256:{}", "f".repeat(64));
    let drift_loads = attempted_loads.clone();
    let same_epoch_drift = registry
        .acquire(
            ParserReadRequest::new(drift).expect("same-epoch drift request"),
            RuntimeEndpointState::Unavailable,
            move || async move {
                drift_loads.fetch_add(1, Ordering::Relaxed);
                Err("same-epoch drift loader must not execute".to_owned())
            },
        )
        .await;
    assert!(same_epoch_drift.is_err());
    assert_eq!(attempted_loads.load(Ordering::Relaxed), 0);
    assert_eq!(registry.resident_generation_count().await, 1);
}

#[tokio::test]
async fn repeated_generation_replacement_keeps_one_registry_entry_per_workspace() {
    let registry = ParserReadAuthorityRegistry::new();
    for epoch in 1..=128_u64 {
        let mut next = generation();
        next.generation_epoch = epoch;
        next.generation_digest = format!("blake3-256:{epoch:064x}");
        next.source_root_digest = format!("blake3-256:{:064x}", epoch + 128);
        let loaded = next.clone();
        registry
            .acquire(
                ParserReadRequest::new(next).expect("monotonic parser generation request"),
                RuntimeEndpointState::Stopped,
                move || async move {
                    Ok(LoadedParserGeneration {
                        generation: loaded,
                        checkpoint_bytes: Arc::from([1_u8]),
                        counters: ParserReadCounters {
                            mapped_bytes: 1,
                            ..ParserReadCounters::default()
                        },
                    })
                },
            )
            .await
            .expect("monotonic generation replacement");
        assert_eq!(registry.resident_generation_count().await, 1);
    }
}
